# ASR Integration Guide

This guide shows how to integrate the validated ASR system into your Dora applications.

## Table of Contents
1. [Basic Integration](#basic-integration)
2. [WebSocket Integration](#websocket-integration)
3. [Cloud Deployment](#cloud-deployment)
4. [Performance Optimization](#performance-optimization)
5. [Multi-Language Support](#multi-language-support)

## Basic Integration

### Python Script Integration

```python
#!/usr/bin/env python3
import os
import librosa
from dora_asr.manager import ASRManager

# Configure for your needs
os.environ['USE_GPU'] = 'true'        # Enable GPU
os.environ['ASR_ENGINE'] = 'funasr'   # Use FunASR for Chinese
os.environ['LANGUAGE'] = 'zh'         # Chinese language

# Initialize ASR
manager = ASRManager()

# Load and transcribe audio
audio, sr = librosa.load("your_audio.wav", sr=16000)
result = manager.transcribe(audio, language='zh')

print(f"Text: {result['text']}")
```

### Dora Node Integration

Create `my_asr_node.py`:

```python
from dora import Node
from dora_asr.manager import ASRManager
import numpy as np

node = Node()
manager = ASRManager(node)

for event in node:
    if event["type"] == "INPUT" and event["id"] == "audio":
        # Convert bytes to numpy array
        audio_bytes = event["value"].to_numpy()
        audio_array = np.frombuffer(audio_bytes, dtype=np.float32)
        
        # Transcribe
        result = manager.transcribe(audio_array)
        
        # Send output
        node.send_output("text", result["text"])
        
        # Optional: send metadata
        node.send_output("language", result.get("language", "unknown"))
```

## WebSocket Integration

### Server Configuration

```yaml
# websocket_asr_dataflow.yml
nodes:
  # WebSocket server
  - id: websocket-server
    operator:
      rust: ../../../rust-nodes/dora-openai-websocket
    outputs:
      - audio
    env:
      PORT: "8123"
      
  # ASR with GPU
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
      
  # Process transcriptions
  - id: text-processor
    custom:
      source: python
      args: process_text.py
    inputs:
      text: asr/text
```

### Client Example

```javascript
// JavaScript WebSocket client
const ws = new WebSocket('ws://localhost:8123');

ws.onopen = () => {
    console.log('Connected to ASR server');
    
    // Send audio data
    navigator.mediaDevices.getUserMedia({ audio: true })
        .then(stream => {
            // Process and send audio chunks
            const mediaRecorder = new MediaRecorder(stream);
            mediaRecorder.ondataavailable = (event) => {
                if (event.data.size > 0) {
                    ws.send(event.data);
                }
            };
            mediaRecorder.start(100); // Send every 100ms
        });
};

ws.onmessage = (event) => {
    const transcription = JSON.parse(event.data);
    console.log('Transcription:', transcription.text);
};
```

## Cloud Deployment

### Docker Configuration

```dockerfile
# Dockerfile for ASR service
FROM nvidia/cuda:12.1.0-runtime-ubuntu22.04

# Install Python and dependencies
RUN apt-get update && apt-get install -y \
    python3.12 \
    python3-pip \
    git \
    git-lfs \
    && rm -rf /var/lib/apt/lists/*

# Install dora-asr
COPY python-nodes/dora-asr /app/dora-asr
WORKDIR /app
RUN pip install -e dora-asr

# Download models
RUN python -c "from dora_asr.manager import ASRManager; m = ASRManager()"

# Environment for GPU
ENV USE_GPU=true
ENV ASR_ENGINE=funasr
ENV LANGUAGE=zh

CMD ["python", "asr_service.py"]
```

### Kubernetes Deployment

```yaml
# asr-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: asr-service
spec:
  replicas: 2
  selector:
    matchLabels:
      app: asr
  template:
    metadata:
      labels:
        app: asr
    spec:
      containers:
      - name: asr
        image: your-registry/dora-asr:latest
        resources:
          limits:
            nvidia.com/gpu: 1
            memory: "4Gi"
          requests:
            nvidia.com/gpu: 1
            memory: "2Gi"
        env:
        - name: USE_GPU
          value: "true"
        - name: ASR_ENGINE
          value: "funasr"
        ports:
        - containerPort: 8080
```

## Performance Optimization

### Batch Processing

```python
from dora_asr.manager import ASRManager
import numpy as np

manager = ASRManager()

# Process multiple audio files efficiently
audio_files = ["file1.wav", "file2.wav", "file3.wav"]
results = []

for audio_file in audio_files:
    audio, sr = librosa.load(audio_file, sr=16000)
    result = manager.transcribe(audio)
    results.append(result)
    
# Manager reuses loaded models for efficiency
```

### Stream Processing

```python
import pyaudio
from dora_asr.manager import ASRManager
import numpy as np

manager = ASRManager()

# Audio stream configuration
CHUNK = 16000  # 1 second chunks
FORMAT = pyaudio.paFloat32
CHANNELS = 1
RATE = 16000

p = pyaudio.PyAudio()
stream = p.open(format=FORMAT,
                channels=CHANNELS,
                rate=RATE,
                input=True,
                frames_per_buffer=CHUNK)

print("Listening... Press Ctrl+C to stop")

try:
    while True:
        # Read audio chunk
        data = stream.read(CHUNK)
        audio_array = np.frombuffer(data, dtype=np.float32)
        
        # Transcribe chunk
        result = manager.transcribe(audio_array)
        if result['text']:
            print(f"Transcription: {result['text']}")
            
except KeyboardInterrupt:
    print("Stopping...")
finally:
    stream.stop_stream()
    stream.close()
    p.terminate()
    manager.cleanup()
```

### GPU Memory Management

```python
import torch
from dora_asr.manager import ASRManager

# Monitor GPU memory
if torch.cuda.is_available():
    print(f"GPU Memory before: {torch.cuda.memory_allocated()/1024**3:.2f} GB")

manager = ASRManager()

# Process audio...
result = manager.transcribe(audio_data)

if torch.cuda.is_available():
    print(f"GPU Memory after: {torch.cuda.memory_allocated()/1024**3:.2f} GB")
    
# Clean up when done
manager.cleanup()
torch.cuda.empty_cache()
```

## Multi-Language Support

### Auto Language Detection

```yaml
# multi_language_dataflow.yml
nodes:
  - id: asr-multilingual
    operator:
      python: ../../../python-nodes/dora-asr
    inputs:
      audio: audio-source/audio
    outputs:
      - text
      - language
    env:
      USE_GPU: "true"
      ASR_ENGINE: "auto"      # Auto-select engine
      LANGUAGE: "auto"        # Auto-detect language
```

### Language-Specific Configuration

```python
from dora_asr.manager import ASRManager

manager = ASRManager()

# Chinese audio - uses FunASR
chinese_result = manager.transcribe(chinese_audio, language='zh')

# English audio - uses Whisper
english_result = manager.transcribe(english_audio, language='en')

# Auto-detect - analyzes audio first
auto_result = manager.transcribe(unknown_audio, language='auto')
print(f"Detected language: {auto_result['language']}")
```

### Custom Language Routing

```python
class CustomASRManager(ASRManager):
    def get_engine_for_language(self, language: str) -> str:
        """Custom engine selection logic"""
        
        # Custom mappings
        custom_mappings = {
            'zh': 'funasr',      # Chinese -> FunASR
            'en': 'whisper',     # English -> Whisper
            'ja': 'whisper',     # Japanese -> Whisper
            'ko': 'whisper',     # Korean -> Whisper
        }
        
        return custom_mappings.get(language, 'whisper')

# Use custom manager
manager = CustomASRManager()
```

## Advanced Features

### Custom Preprocessing

```python
import librosa
import numpy as np
from dora_asr.manager import ASRManager

def preprocess_audio(audio, sr=16000):
    """Custom audio preprocessing"""
    
    # Noise reduction
    audio = librosa.effects.preemphasis(audio)
    
    # Normalize
    audio = audio / np.max(np.abs(audio))
    
    # Remove silence
    audio, _ = librosa.effects.trim(audio, top_db=20)
    
    return audio

# Use with ASR
manager = ASRManager()
audio, sr = librosa.load("noisy_audio.wav", sr=16000)
audio = preprocess_audio(audio)
result = manager.transcribe(audio)
```

### Confidence Scoring

```python
from dora_asr.manager import ASRManager

# Enable confidence scoring
os.environ['ENABLE_CONFIDENCE_SCORE'] = 'true'

manager = ASRManager()
result = manager.transcribe(audio_data)

# Check confidence
if 'confidence' in result:
    if result['confidence'] > 0.8:
        print(f"High confidence: {result['text']}")
    else:
        print(f"Low confidence: {result['text']} ({result['confidence']:.2f})")
```

### Custom Post-Processing

```python
import re
from dora_asr.manager import ASRManager

def post_process_text(text: str, language: str) -> str:
    """Clean up transcription text"""
    
    if language == 'zh':
        # Remove extra spaces in Chinese
        text = re.sub(r'\s+', '', text)
        # Add proper punctuation
        text = text.replace('，，', '，').replace('。。', '。')
        
    elif language == 'en':
        # Capitalize sentences
        text = '. '.join(s.capitalize() for s in text.split('. '))
        # Fix common mistakes
        text = text.replace(' i ', ' I ')
        
    return text.strip()

# Use with ASR
manager = ASRManager()
result = manager.transcribe(audio_data)
result['text'] = post_process_text(result['text'], result['language'])
```

## Monitoring and Logging

### Performance Metrics

```python
import time
from dora_asr.manager import ASRManager

class MonitoredASRManager(ASRManager):
    def transcribe(self, audio_array, language=None):
        start_time = time.time()
        
        # Original transcription
        result = super().transcribe(audio_array, language)
        
        # Add metrics
        elapsed = time.time() - start_time
        audio_duration = len(audio_array) / self.config.SAMPLE_RATE
        
        result['metrics'] = {
            'processing_time': elapsed,
            'audio_duration': audio_duration,
            'rtf': elapsed / audio_duration,
            'speed': audio_duration / elapsed,
        }
        
        # Log metrics
        self.send_log("INFO", f"RTF: {result['metrics']['rtf']:.3f}")
        
        return result
```

### Error Handling

```python
from dora_asr.manager import ASRManager
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def safe_transcribe(audio_data, max_retries=3):
    """Transcribe with retry logic"""
    
    manager = ASRManager()
    
    for attempt in range(max_retries):
        try:
            result = manager.transcribe(audio_data)
            return result
            
        except Exception as e:
            logger.error(f"Attempt {attempt + 1} failed: {e}")
            
            if attempt == max_retries - 1:
                # Final attempt failed
                return {
                    'text': '',
                    'error': str(e),
                    'language': 'unknown'
                }
            
            # Wait before retry
            time.sleep(2 ** attempt)
    
    return None
```

## Best Practices

### 1. Resource Management
- Always call `manager.cleanup()` when done
- Use context managers for automatic cleanup
- Monitor GPU memory usage

### 2. Performance
- Use GPU for production deployments
- Batch process when possible
- Cache model instances

### 3. Quality
- Preprocess audio (normalize, denoise)
- Use appropriate sample rate (16kHz)
- Select right engine for language

### 4. Deployment
- Use Docker for consistency
- Set resource limits
- Implement health checks

### 5. Monitoring
- Log performance metrics
- Track error rates
- Monitor resource usage

## Summary

This integration guide provides:
- ✅ Basic Python integration
- ✅ Dora node examples
- ✅ WebSocket server setup
- ✅ Cloud deployment configs
- ✅ Performance optimization tips
- ✅ Multi-language support
- ✅ Advanced features
- ✅ Monitoring strategies

Use these examples as templates for your specific use case!