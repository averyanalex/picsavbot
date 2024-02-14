use std::{
    collections::{BTreeMap, HashMap},
    env,
    future::Future,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use db::Db;
use reqwest::Client;
use sentry::protocol::Value;
use serde::{Deserialize, Serialize};
use teloxide::{
    adaptors::{throttle::Limits, Throttle},
    macros::BotCommands,
    net::Download,
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InlineQueryResultCachedPhoto,
        InlineQueryResultCachedSticker, InputMessageContent, InputMessageContentText, ParseMode,
        User,
    },
    utils::command::BotCommands as _,
};
use tokio::sync::Mutex as AsyncMutex;
use tracing::*;
use tracing_subscriber::prelude::*;

use entities::sea_orm_active_enums::MediaType;

mod db;

type Bot = Throttle<teloxide::Bot>;

fn main() -> Result<()> {
    std::env::set_var("RUST_BACKTRACE", "1");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer().with_filter(
                tracing_subscriber::filter::LevelFilter::from_str(
                    &std::env::var("RUST_LOG").unwrap_or_else(|_| String::from("info")),
                )
                .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO),
            ),
        )
        .with(
            sentry_tracing::layer().event_filter(|md| match *md.level() {
                Level::TRACE => sentry_tracing::EventFilter::Ignore,
                _ => sentry_tracing::EventFilter::Breadcrumb,
            }),
        )
        .try_init()
        .unwrap();

    let _sentry_guard = match std::env::var("SENTRY_DSN") {
        Ok(d) => {
            let guard = sentry::init((
                d,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    attach_stacktrace: true,
                    traces_sample_rate: 1.0,
                    ..Default::default()
                },
            ));
            Some(guard)
        }
        Err(e) => {
            warn!("can't get SENTRY_DSN: {:?}", e);
            None
        }
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(_main())
}

async fn _main() -> Result<()> {
    tracing::info!("Starting bot...");
    let bot = teloxide::Bot::from_env().throttle(Limits::default());

    let handler = dptree::entry()
        .branch(Update::filter_message().branch(dptree::endpoint(handle_message)))
        .branch(
            Update::filter_chosen_inline_result().branch(dptree::endpoint(handle_chosen_inline)),
        )
        .branch(Update::filter_inline_query().branch(dptree::endpoint(handle_inline_query)));

    let db = Arc::new(Db::new().await?);
    let ai = Arc::new(Ai::default());
    let translator = Arc::new(Translator::new()?);

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![db, ai, translator])
        .enable_ctrlc_handler()
        // .worker_queue_size(2)
        .build()
        .dispatch()
        .await;

    Ok(())
}

#[derive(Default)]
struct Ai {
    client: Client,
    lock: AsyncMutex<()>,
}

#[derive(Deserialize, Debug)]
struct EmbeddingsResponse {
    embeddings: Vec<Vec<f32>>,
}

impl EmbeddingsResponse {
    fn get_one(self) -> Result<Vec<f32>> {
        self.embeddings
            .into_iter()
            .next()
            .context("empty embeddings")
    }
}

impl Ai {
    async fn images_embeddings(&self, images: Vec<Vec<u8>>) -> Result<EmbeddingsResponse> {
        let lock = self.lock.lock().await;

        let mut form = reqwest::multipart::Form::new();

        for image in images {
            let file_part = reqwest::multipart::Part::bytes(image).file_name("image");
            form = form.part("files", file_part);
        }

        let req = self
            .client
            .post("http://127.0.0.1:8526/images")
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;
        drop(lock);

        Ok(req.json().await?)
    }

    async fn text_embeddings(&self, texts: Vec<String>) -> Result<EmbeddingsResponse> {
        #[derive(Serialize, Debug)]
        struct TextRequest {
            texts: Vec<String>,
        }

        let lock = self.lock.lock().await;

        let res = self
            .client
            .post("http://127.0.0.1:8526/texts")
            .json(&TextRequest { texts })
            .send()
            .await?
            .error_for_status()?;
        drop(lock);

        Ok(res.json().await?)
    }
}

struct Translator {
    ycl_api_key: String,
    ycl_folder: String,
    client: Client,
    cache: Mutex<HashMap<String, String>>,
}

impl Translator {
    fn new() -> Result<Self> {
        Ok(Self {
            ycl_api_key: env::var("YCL_API_KEY")?,
            ycl_folder: env::var("YCL_FOLDER")?,
            client: Client::new(),
            cache: Mutex::default(),
        })
    }

    async fn translate(&self, text: String) -> Result<String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct TranslateRequest {
            folder_id: String,
            texts: [String; 1],
            target_language_code: String,
            source_language_code: String,
            speller: bool,
        }

        #[derive(Deserialize)]
        struct TranslateResponse {
            translations: [Translation; 1],
        }

        #[derive(Deserialize)]
        struct Translation {
            text: String,
        }

        let cached_translation = self.cache.lock().unwrap().get(&text).cloned();
        if let Some(translation) = cached_translation {
            Ok(translation)
        } else {
            let res: TranslateResponse = self
                .client
                .post("https://translate.api.cloud.yandex.net/translate/v2/translate")
                .header("Authorization", format!("Api-Key {}", self.ycl_api_key))
                .json(&TranslateRequest {
                    folder_id: self.ycl_folder.clone(),
                    texts: [text.clone()],
                    target_language_code: "en".into(),
                    source_language_code: "ru".into(),
                    speller: true,
                })
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            let translation = res.translations.into_iter().map(|t| t.text).next().unwrap();
            self.cache.lock().unwrap().insert(text, translation.clone());
            Ok(translation)
        }
    }
}

#[derive(BotCommands)]
enum Command {
    #[command(rename = "reindex")]
    Reindex,
    // User(i64),
}

async fn handle_inline_query(
    db: Arc<Db>,
    ai: Arc<Ai>,
    translator: Arc<Translator>,
    bot: Bot,
    query: InlineQuery,
) -> Result<()> {
    try_handle(&query.from, &bot, async {
        let offset: Option<u64> = if let Ok(offset) = query.offset.parse() {
            Some(offset)
        } else {
            None
        };

        let images: Vec<_> = if query.query.is_empty() {
            db.get_most_used_images(query.from.id.0.try_into().unwrap(), offset)
                .await?
        } else {
            let translated_text = translator.translate(query.query).await?;

            let embeddings = ai.text_embeddings(vec![translated_text]).await?;
            let embedding = embeddings.get_one()?;

            db.search_images(query.from.id.0.try_into().unwrap(), embedding, offset)
                .await?
        };

        let images_len = images.len();
        let results: Vec<_> = images
            .into_iter()
            .take(50)
            .map(|i| match i.media_type {
                MediaType::Photo => InlineQueryResult::CachedPhoto(
                    InlineQueryResultCachedPhoto::new(i.id.to_string(), i.file_id),
                ),
                MediaType::Sticker => InlineQueryResult::CachedSticker(
                    InlineQueryResultCachedSticker::new(i.id.to_string(), i.file_id),
                ),
            })
            .collect();

        if results.is_empty() {
            bot.answer_inline_query(
                query.id.clone(),
                vec![InlineQueryResult::Article(
                    InlineQueryResultArticle::new(
                        "howtouse",
                        "Напишите боту @picsavbot",
                        InputMessageContent::Text(InputMessageContentText::new(
                            bot.get_me().await?.tme_url(),
                        )),
                    )
                    .description("Напишите боту @picsavbot, чтобы начать работу"),
                )],
            )
            .switch_pm_text("Чтобы начать работу, сохраните в @picsavbot несколько изображений")
            .switch_pm_parameter(query.id)
            .cache_time(0)
            .await?;
        } else {
            let mut req = bot.answer_inline_query(query.id, results).cache_time(0);
            if images_len >= 51 {
                let new_offset = if let Some(offset) = offset {
                    offset + 50
                } else {
                    50
                };
                req = req.next_offset(new_offset.to_string());
            }
            req.await?;
        }

        db.update_user(query.from.id.0.try_into().unwrap()).await?;
        Ok(())
    })
    .await
}

async fn handle_chosen_inline(db: Arc<Db>, chosen: ChosenInlineResult) -> Result<()> {
    if let Ok(image) = chosen.result_id.parse() {
        db.increment_image_uses(image, chosen.from.id.0.try_into().unwrap())
            .await?;
    }
    Ok(())
}

async fn handle_message(db: Arc<Db>, ai: Arc<Ai>, bot: Bot, msg: Message) -> Result<()> {
    if let Some(from) = msg.from() {
        try_handle(from, &bot, async {
            db.update_user(msg.chat.id.0).await?;

            #[allow(clippy::manual_map)]
            if let Some((media_type, file)) = if let Some([.., photo]) = msg.photo() {
                Some((MediaType::Photo, photo.file.clone()))
            } else if let Some(sticker) = msg.sticker() {
                if sticker.is_raster() {
                    Some((MediaType::Sticker, sticker.file.clone()))
                } else {
                    bot.send_message(msg.chat.id, "В данный момент возможно сохранение только обычных, неанимированных стикеров")
                        .reply_to_message_id(msg.id)
                        .await?;
                    return Ok(());
                }
            } else {
                None
            } {
                if db.delete_image(msg.chat.id.0, file.unique_id.clone()).await? {
                    bot.send_message(msg.chat.id, "Изображение удалено!").reply_to_message_id(msg.id).await?;
                } else {
                    let file = bot.get_file(&file.id).await?;
                    let mut dst = Vec::new();
                    bot.download_file(&file.path, &mut dst).await?;

                    let embeddings = ai.images_embeddings(vec![dst]).await?;
                    let embedding = embeddings.get_one()?;

                    db.create_image(msg.chat.id.0, embedding, file.id.clone(), file.unique_id.clone(), media_type)
                        .await?;

                    bot.send_message(
                        msg.chat.id,
                        "Ваше изображение/стикер сохранено\\!\n\nТеперь вы можете найти и \
                    отправить его, написав `@picsavbot \\[описание изображения по-русски\\]` в любом чате\\.\n\nА чтобы его удалить, \
                    отправьте его ещё раз с помощью `@picsavbot \\[описание изображения по-русски\\]`\\.",
                    )
                    .parse_mode(ParseMode::MarkdownV2)
                    .reply_to_message_id(msg.id)
                    .await?;
                }
            } else {
                if msg.chat.id.0 == 1004106925 {
                    if let Some(text) = msg.text() {
                        if let Ok(cmd) = Command::parse(text, bot.get_me().await?.username()) {
                            match cmd {
                                Command::Reindex => {
                                    for image in db.get_all_images().await? {
                                        let file = bot.get_file(image.file_id).await?;
                                        let mut dst = Vec::new();
                                        bot.download_file(&file.path, &mut dst).await?;

                                        let embeddings = ai.images_embeddings(vec![dst]).await?;
                                        let embedding = embeddings.get_one()?;

                                        db.update_image(image.id, embedding).await?;
                                        info!("reindexed {}", image.id);
                                    }
                                }
                                // Command::User(_) => {},
                            }
                        }
                    }
                }
                bot.send_message(
                    msg.chat.id,
                    "Чтобы начать работу, отправьте боту картинку или стикер, и бот \
                её сохранит. После этого бот объяснит, как искать и отправлять сохранённые пикчи и стикеры.",
                )
                .await?;
            };
            Ok(())
        })
        .await
    } else {
        Ok(())
    }
}

async fn try_handle(
    user: &User,
    bot: &Bot,
    handle: impl Future<Output = Result<()>>,
) -> Result<()> {
    sentry::start_session();
    sentry::configure_scope(|scope| {
        let mut map = BTreeMap::new();
        map.insert(
            "first_name".to_owned(),
            Value::from(user.first_name.clone()),
        );
        map.insert("last_name".to_owned(), Value::from(user.last_name.clone()));
        scope.set_user(Some(sentry::User {
            id: Some(user.id.0.to_string()),
            username: user.username.clone(),
            other: map,
            ..Default::default()
        }));
    });

    if let Err(e) = handle.await {
        sentry_anyhow::capture_anyhow(&e);
        bot.send_message(
            ChatId::from(user.id),
            format!("Произошла неизвестная ошибка: {e}"),
        )
        .await
        .ok();
    }

    sentry::end_session();

    Ok(())
}
