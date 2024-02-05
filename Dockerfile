FROM python:3.11 AS builder
RUN python -m pip install --no-cache-dir poetry==1.7
COPY poetry.lock pyproject.toml ./
RUN poetry export --without-hashes --without dev -f requirements.txt -o requirements.txt

FROM python:3.11
WORKDIR /app
COPY --from=builder requirements.txt ./
RUN python -m pip install --no-cache-dir -r requirements.txt
COPY download.py ./
RUN python download.py
COPY app.py ./
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8526"]
