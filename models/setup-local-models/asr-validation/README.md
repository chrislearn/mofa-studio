# ASR Validation and Testing Suite

This directory contains comprehensive testing and validation tools for the dora-asr node, including GPU acceleration benchmarks and environment configuration tests.

## Overview

The dora-asr node supports multiple ASR engines (Whisper, FunASR) with GPU acceleration capabilities. This validation suite helps you:

1. Test ASR functionality with sample audio files
2. Benchmark CPU vs GPU performance
3. Validate environment variable configuration
4. Verify proper engine selection and device management

## Quick Start

### 1. Basic ASR Test

Test basic ASR functionality with the provided Chinese audio sample:

```bash
# Test with CPU
USE_GPU=false python test_basic_asr.py

# Test with GPU
USE_GPU=true python test_basic_asr.py
```

### 2. Performance Benchmark

Compare performance between CPU and GPU modes:

```bash
# Run full benchmark
python benchmark_gpu.py --audio test.wav

# Skip original implementation comparison
python benchmark_gpu.py --audio test.wav --skip-original

# Test CPU only (no GPU comparison)
python benchmark_gpu.py --audio test.wav --cpu-only
```

### 3. Environment Control Validation

Verify that environment variables correctly control GPU usage:

```bash
# Test environment variable parsing and engine selection
python test_gpu_env_control.py

# Test with actual transcription
python validate_gpu_switching.py
```

## Test Files

### Audio Samples

- **test.wav**: 17.35 seconds of Chinese speech
  - Sample rate: 16kHz
  - Content: "你好吗？请你告诉我怎么坐公共汽车去北京动物园？请你告诉我梅菜扣肉怎么做和涮羊？怎么做？"
  - Perfect for testing Chinese ASR engines (FunASR)

### Test Scripts

1. **test_basic_asr.py**: Simple ASR test with audio file
2. **benchmark_gpu.py**: Comprehensive performance benchmarking
3. **test_gpu_env_control.py**: Environment variable validation
4. **validate_gpu_switching.py**: Runtime GPU/CPU switching test
5. **test_dataflow_gpu.py**: Test ASR in Dora dataflow context

## Configuration Guide

### Environment Variables

| Variable | Values | Description | Default |
|----------|--------|-------------|---------|
| `USE_GPU` | `"true"/"false"` | Enable GPU acceleration | `"false"` |
| `ASR_ENGINE` | `"funasr"/"whisper"/"auto"` | Select ASR engine | `"auto"` |
| `LANGUAGE` | `"zh"/"en"/"auto"` | Target language | `"auto"` |
| `ASR_MODELS_DIR` | Path | Model directory | `~/.dora/models/asr` |
| `LOG_LEVEL` | `"DEBUG"/"INFO"/"WARNING"/"ERROR"` | Logging level | `"INFO"` |
| `ENABLE_PUNCTUATION` | `"true"/"false"` | Add punctuation | `"true"` |
| `MIN_AUDIO_DURATION` | Float (seconds) | Minimum audio length | `"0.5"` |

### YAML Dataflow Configuration

Example configurations for different scenarios:

#### GPU-Enabled Chinese ASR
```yaml
nodes:
  - id: asr
    operator:
      python: ../../../python-nodes/dora-asr
    inputs:
      audio: audio-source/audio
    outputs:
      - text
    env:
      USE_GPU: "true"
      ASR_ENGINE: "funasr"
      LANGUAGE: "zh"
      ENABLE_PUNCTUATION: "true"
```

#### CPU-Only English ASR
```yaml
nodes:
  - id: asr
    operator:
      python: ../../../python-nodes/dora-asr
    inputs:
      audio: audio-source/audio
    outputs:
      - text
    env:
      USE_GPU: "false"
      ASR_ENGINE: "whisper"
      LANGUAGE: "en"
```

#### Auto-Detection Mode
```yaml
nodes:
  - id: asr
    operator:
      python: ../../../python-nodes/dora-asr
    inputs:
      audio: audio-source/audio
    outputs:
      - text
    env:
      USE_GPU: "true"
      ASR_ENGINE: "auto"  # Automatically select based on language
      LANGUAGE: "auto"    # Detect language from audio
```

## Performance Benchmarks

Based on testing with RTX 4090 GPU:

### Chinese ASR (FunASR) - 17.35s Audio

| Configuration | Processing Time | RTF | Real-time Speed | Notes |
|--------------|-----------------|-----|-----------------|-------|
| Original ONNX (CPU) | 0.640s | 0.037 | 27.1x | Baseline |
| PyTorch CPU | 0.955s | 0.055 | 18.2x | GPU engine on CPU |
| PyTorch CUDA | 0.282s | 0.016 | 61.6x | **2.27x speedup** |

### Key Findings

1. **GPU Acceleration**: 2.27x faster processing with GPU
2. **Memory Usage**: Only 2GB VRAM required
3. **Quality**: Identical transcription accuracy across all modes
4. **Real-time Factor**: GPU achieves 61.6x real-time speed

## Validation Procedures

### 1. Pre-Installation Check

```bash
# Check Python environment
python --version  # Should be 3.12

# Check CUDA availability
python -c "import torch; print(f'CUDA: {torch.cuda.is_available()}')"

# Check installed packages
pip list | grep -E "dora-asr|funasr|whisper"
```

### 2. Model Availability

```bash
# Check model directory
ls ~/.dora/models/asr/funasr/

# Expected models:
# - speech_seaco_paraformer_large_asr_nat-zh-cn-16k-common-vocab8404-pytorch/
# - punc_ct-transformer_cn-en-common-vocab471067-large/
```

### 3. Engine Selection Test

```python
# test_engine_selection.py
import os
from dora_asr.manager import ASRManager

# Test CPU mode
os.environ['USE_GPU'] = 'false'
manager_cpu = ASRManager()
print(f"CPU mode: {manager_cpu._engine_classes['funasr'].__name__}")

# Test GPU mode
os.environ['USE_GPU'] = 'true'
manager_gpu = ASRManager()
print(f"GPU mode: {manager_gpu._engine_classes['funasr'].__name__}")
```

### 4. Transcription Quality Test

```python
# test_quality.py
import librosa
from dora_asr.manager import ASRManager

# Load audio
audio, sr = librosa.load("test.wav", sr=16000)

# Test transcription
manager = ASRManager()
result = manager.transcribe(audio, language='zh')
print(f"Transcription: {result['text']}")

# Expected output:
# "你好吗？请你告诉我怎么坐公共汽车去北京动物园？请你告诉我梅菜扣肉怎么做和涮羊？怎么做？"
```

## Troubleshooting

### Issue: GPU not detected

```bash
# Check CUDA installation
nvidia-smi

# Check PyTorch CUDA support
python -c "import torch; print(torch.version.cuda)"

# Reinstall PyTorch with CUDA
pip uninstall torch
pip install torch --index-url https://download.pytorch.org/whl/cu121
```

### Issue: Models not loading

```bash
# Check model files are not just Git LFS pointers
ls -lh ~/.dora/models/asr/funasr/*/

# If files are small (< 1MB), pull LFS files
cd ~/.dora/models/asr/funasr/speech_seaco_paraformer_large_asr_nat-zh-cn-16k-common-vocab8404-pytorch
git lfs pull
```

### Issue: Wrong engine selected

```python
# Debug engine selection
import os
os.environ['USE_GPU'] = 'true'

from dora_asr.config import ASRConfig
from dora_asr.manager import ASRManager

config = ASRConfig()
print(f"Config USE_GPU: {config.USE_GPU}")

manager = ASRManager()
print(f"Available engines: {list(manager._engine_classes.keys())}")
print(f"FunASR engine class: {manager._engine_classes['funasr'].__name__}")
```

### Issue: Performance not as expected

1. Check GPU utilization:
```bash
# Monitor GPU usage during transcription
watch -n 0.5 nvidia-smi
```

2. Run benchmark for detailed metrics:
```bash
python benchmark_gpu.py --audio test.wav
```

3. Check for thermal throttling:
```bash
nvidia-smi -q -d PERFORMANCE
```

## Integration Examples

### 1. Simple Python Script

```python
#!/usr/bin/env python3
import os
import librosa
from dora_asr.manager import ASRManager

# Enable GPU
os.environ['USE_GPU'] = 'true'

# Initialize ASR
manager = ASRManager()

# Load and transcribe audio
audio, sr = librosa.load("your_audio.wav", sr=16000)
result = manager.transcribe(audio, language='zh')

print(f"Text: {result['text']}")
print(f"Language: {result['language']}")
```

### 2. Dora Node Integration

```python
# asr_node.py
from dora import Node
from dora_asr.manager import ASRManager
import numpy as np

node = Node()
manager = ASRManager(node)

for event in node:
    if event["type"] == "INPUT":
        audio_bytes = event["value"].to_numpy()
        audio_array = np.frombuffer(audio_bytes, dtype=np.float32)
        
        result = manager.transcribe(audio_array)
        node.send_output("text", result["text"])
```

### 3. WebSocket Server Integration

```yaml
# dataflow_websocket.yml
nodes:
  - id: websocket-server
    operator:
      rust: ../../../rust-nodes/dora-openai-websocket
    outputs:
      - audio
    
  - id: asr
    operator:
      python: ../../../python-nodes/dora-asr
    inputs:
      audio: websocket-server/audio
    outputs:
      - text
    env:
      USE_GPU: "true"
      ASR_ENGINE: "funasr"
      LANGUAGE: "auto"
```
