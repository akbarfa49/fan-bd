#!/bin/sh
uvicorn --dir ocrpy ocr_server:app --host 127.0.0.1 --port 42069