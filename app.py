from PIL import Image
from io import BytesIO
from fastapi import FastAPI, UploadFile, Response, Request
from pydantic import BaseModel

from transformers import CLIPProcessor, CLIPModel

model = CLIPModel.from_pretrained("laion/CLIP-ViT-H-14-laion2B-s32B-b79K")
processor = CLIPProcessor.from_pretrained("laion/CLIP-ViT-H-14-laion2B-s32B-b79K")

app = FastAPI()


class EmbeddingsResponse(BaseModel):
    embeddings: list[list[float]]


@app.post("/images")
async def image_embeddings(files: list[UploadFile]):
    images = []
    for file in files:
        contents = await file.read()
        img = Image.open(BytesIO(contents))
        images.append(img)

    inputs = processor(images=images, return_tensors="pt", padding=True)
    features = model.get_image_features(**inputs)
    features = features / features.norm(p=2, dim=-1, keepdim=True)

    return EmbeddingsResponse(embeddings=features)


class TextRequest(BaseModel):
    texts: list[str]


@app.post("/texts")
async def text_embeddings(req: TextRequest) -> EmbeddingsResponse:
    inputs = processor(text=req.texts, return_tensors="pt", padding=True)
    features = model.get_text_features(**inputs)
    features = features / features.norm(p=2, dim=-1, keepdim=True)

    return EmbeddingsResponse(embeddings=features)
