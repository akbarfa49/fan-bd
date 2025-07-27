import cv2, time, numpy as np, onnxruntime
from fastapi import FastAPI, File, UploadFile, Form
from onnxocr.onnx_paddleocr import ONNXPaddleOcr

# 1) Load **mobile** model explicitly
sess_opts = onnxruntime.SessionOptions()
sess_opts.intra_op_num_threads = 1
sess_opts.inter_op_num_threads = 1

ocr = ONNXPaddleOcr(
    det_model_path=None,
    cls_model_path=None,
    use_angle_cls=False,
    sess_options=sess_opts,
    use_dml=True,
    use_gpu=False,
)

app = FastAPI()

@app.post("/ocr")
async def run_ocr(
    width: int = Form(...),
    height: int = Form(...),
    file: UploadFile = File(...)
):
    content = await file.read()
    np_img = np.frombuffer(content, dtype=np.uint8).reshape((height, width,3))

    result = ocr.ocr(np_img)

    serialized = []
    for v, (text, score) in result[0]:
        xs, ys = zip(*v)
        serialized.append({
            'area': {
                'left': int(min(xs)), 'top': int(min(ys)),
                'right': int(max(xs)), 'bottom': int(max(ys))
            },
            'text': text, 'score': float(score)
        })
    return {"result": serialized}
