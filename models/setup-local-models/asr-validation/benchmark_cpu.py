#!/usr/bin/env python3
"""
CPU Benchmark for FunASR: Compare PyTorch vs ONNX backends
Designed for Mac and other CPU-only systems
"""

import os
import sys
import time
import json
import argparse
import numpy as np
from pathlib import Path
from typing import Dict, List, Optional
import librosa

# Add dora-asr to path
asr_path = Path(__file__).parent.parent.parent.parent / "python-nodes" / "dora-asr"
if asr_path.exists():
    sys.path.insert(0, str(asr_path))

# Configure environment for CPU-only
os.environ['USE_GPU'] = 'false'
os.environ['ASR_ENGINE'] = 'funasr'
os.environ['ASR_MODELS_DIR'] = str(Path.home() / ".dora" / "models" / "asr")


def check_backend_availability():
    """Check which FunASR backends are available"""
    backends = {
        'pytorch': False,
        'onnx': False
    }

    try:
        from funasr import AutoModel
        backends['pytorch'] = True
        print("✅ FunASR PyTorch backend available")
    except ImportError:
        print("❌ FunASR PyTorch backend not available")
        print("   Install: pip install funasr")

    try:
        from funasr_onnx import SeacoParaformer
        backends['onnx'] = True
        print("✅ FunASR ONNX backend available")
    except ImportError:
        print("❌ FunASR ONNX backend not available")
        print("   Install: pip install funasr-onnx")

    return backends


def benchmark_pytorch_cpu(audio_path: str, num_runs: int = 3) -> Optional[Dict]:
    """Benchmark PyTorch CPU backend"""
    print("\n" + "="*70)
    print("Benchmarking FunASR PyTorch CPU Backend")
    print("="*70)

    try:
        from funasr import AutoModel
    except ImportError:
        print("⚠️  PyTorch backend not available, skipping")
        return None

    # Load audio
    audio_data, sr = librosa.load(audio_path, sr=16000)
    audio_duration = len(audio_data) / sr
    print(f"Audio duration: {audio_duration:.2f}s")

    # Model paths
    models_dir = Path.home() / ".dora" / "models" / "asr" / "funasr"
    asr_model_path = models_dir / "speech_seaco_paraformer_large_asr_nat-zh-cn-16k-common-vocab8404-pytorch"
    punc_model_path = models_dir / "punc_ct-transformer_cn-en-common-vocab471067-large"

    if not asr_model_path.exists():
        print(f"❌ Model not found: {asr_model_path}")
        return None

    # Initialize models
    print("Initializing PyTorch models...")
    init_start = time.time()

    asr_model = AutoModel(
        model=str(asr_model_path),
        device="cpu",
        disable_update=True,
        disable_log=True
    )

    punc_model = None
    if punc_model_path.exists():
        punc_model = AutoModel(
            model=str(punc_model_path),
            device="cpu",
            disable_update=True,
            disable_log=True
        )

    init_time = time.time() - init_start
    print(f"Initialization time: {init_time:.2f}s")

    # Warmup
    print("Warming up...")
    warmup_start = time.time()
    _ = asr_model.generate(input=audio_data[:16000], batch_size_s=300)
    warmup_time = time.time() - warmup_start
    print(f"Warmup time: {warmup_time:.2f}s")

    # Run benchmark
    print(f"Running {num_runs} transcription(s)...")
    times = []
    transcriptions = []

    for i in range(num_runs):
        start = time.time()

        # ASR
        result = asr_model.generate(
            input=audio_data,
            batch_size_s=300,
            hotword='魔搭'
        )

        # Extract text
        if isinstance(result, list) and len(result) > 0:
            text = result[0].get("text", "")
        else:
            text = str(result)

        # Add punctuation
        if punc_model and text:
            punc_result = punc_model.generate(input=text)
            if isinstance(punc_result, list) and len(punc_result) > 0:
                text = punc_result[0].get("text", text)

        elapsed = time.time() - start
        times.append(elapsed)
        transcriptions.append(text)

        rtf = elapsed / audio_duration
        print(f"  Run {i+1}: {elapsed:.3f}s (RTF: {rtf:.3f})")

    avg_time = np.mean(times)
    std_time = np.std(times)
    rtf = avg_time / audio_duration

    return {
        'backend': 'PyTorch',
        'device': 'cpu',
        'init_time': init_time,
        'warmup_time': warmup_time,
        'audio_duration': audio_duration,
        'num_runs': num_runs,
        'times': times,
        'avg_time': avg_time,
        'std_time': std_time,
        'min_time': min(times),
        'max_time': max(times),
        'rtf': rtf,
        'speed_x': 1.0 / rtf,
        'transcription': transcriptions[0] if transcriptions else ''
    }


def benchmark_onnx_cpu(audio_path: str, num_runs: int = 3) -> Optional[Dict]:
    """Benchmark ONNX CPU backend"""
    print("\n" + "="*70)
    print("Benchmarking FunASR ONNX CPU Backend")
    print("="*70)

    try:
        from funasr_onnx import SeacoParaformer, CT_Transformer
    except ImportError:
        print("⚠️  ONNX backend not available, skipping")
        return None

    import re

    # Load audio
    audio_data, sr = librosa.load(audio_path, sr=16000)
    audio_duration = len(audio_data) / sr
    print(f"Audio duration: {audio_duration:.2f}s")

    # Model paths
    models_dir = Path.home() / ".dora" / "models" / "asr" / "funasr"
    asr_model_path = models_dir / "speech_seaco_paraformer_large_asr_nat-zh-cn-16k-common-vocab8404-pytorch"
    punc_model_path = models_dir / "punc_ct-transformer_cn-en-common-vocab471067-large"

    if not asr_model_path.exists():
        print(f"❌ Model not found: {asr_model_path}")
        return None

    # Check for ONNX model files
    onnx_model = asr_model_path / "model_quant.onnx"
    if not onnx_model.exists():
        onnx_model = asr_model_path / "model.onnx"

    if not onnx_model.exists():
        print(f"❌ ONNX model not found in {asr_model_path}")
        print("   Run: python convert_to_onnx.py to convert models")
        return None

    use_quantized = "quant" in str(onnx_model)

    # Initialize models
    print(f"Initializing ONNX models (quantized: {use_quantized})...")
    init_start = time.time()

    asr_model = SeacoParaformer(
        str(asr_model_path),
        quantize=use_quantized,
        device_id="-1"  # CPU
    )

    punc_model = None
    if punc_model_path.exists():
        punc_onnx = punc_model_path / "model_quant.onnx"
        if not punc_onnx.exists():
            punc_onnx = punc_model_path / "model.onnx"

        if punc_onnx.exists():
            use_punc_quantized = "quant" in str(punc_onnx)
            punc_model = CT_Transformer(
                str(punc_model_path),
                quantize=use_punc_quantized,
                device_id="-1"
            )

    init_time = time.time() - init_start
    print(f"Initialization time: {init_time:.2f}s")

    # Warmup
    print("Warming up...")
    warmup_start = time.time()
    _ = asr_model(wav_content=audio_data[:16000], hotwords='')
    warmup_time = time.time() - warmup_start
    print(f"Warmup time: {warmup_time:.2f}s")

    # Run benchmark
    print(f"Running {num_runs} transcription(s)...")
    times = []
    transcriptions = []

    for i in range(num_runs):
        start = time.time()

        # ASR
        segments = asr_model(wav_content=audio_data, hotwords='')

        # Process segments
        text_parts = []
        for segment in segments:
            text = segment.get('text', '')
            if text:
                text_parts.append(text)

        text = ' '.join(text_parts)

        # Add punctuation
        if punc_model and text:
            text = punc_model(text=text)

        # Clean up
        text = re.sub(r'\s+', ' ', text).strip()

        elapsed = time.time() - start
        times.append(elapsed)
        transcriptions.append(text)

        rtf = elapsed / audio_duration
        print(f"  Run {i+1}: {elapsed:.3f}s (RTF: {rtf:.3f})")

    avg_time = np.mean(times)
    std_time = np.std(times)
    rtf = avg_time / audio_duration

    return {
        'backend': 'ONNX',
        'device': 'cpu',
        'quantized': use_quantized,
        'init_time': init_time,
        'warmup_time': warmup_time,
        'audio_duration': audio_duration,
        'num_runs': num_runs,
        'times': times,
        'avg_time': avg_time,
        'std_time': std_time,
        'min_time': min(times),
        'max_time': max(times),
        'rtf': rtf,
        'speed_x': 1.0 / rtf,
        'transcription': transcriptions[0] if transcriptions else ''
    }


def print_comparison(results: List[Dict]):
    """Print comparison table"""
    print("\n" + "="*80)
    print("CPU BENCHMARK RESULTS")
    print("="*80)

    # Header
    print(f"\n{'Backend':<12} {'Device':<8} {'Avg Time':<10} {'Min/Max':<12} {'RTF':<8} {'Speed':<10} {'Quantized'}")
    print("-"*80)

    # Results
    for r in results:
        if r is None:
            continue

        min_max = f"{r['min_time']:.3f}/{r['max_time']:.3f}s"
        quantized = str(r.get('quantized', 'N/A'))

        print(f"{r['backend']:<12} {r['device']:<8} {r['avg_time']:.3f}s     "
              f"{min_max:<12} {r['rtf']:.3f}    {r['speed_x']:.1f}x      {quantized}")

    # Performance comparison
    if len(results) == 2 and all(r is not None for r in results):
        print("\n" + "="*80)
        print("PERFORMANCE COMPARISON")
        print("="*80)

        pytorch_result = results[0]
        onnx_result = results[1]

        speedup = pytorch_result['avg_time'] / onnx_result['avg_time']

        print(f"\nPyTorch CPU vs ONNX CPU:")
        print(f"  • PyTorch time: {pytorch_result['avg_time']:.3f}s")
        print(f"  • ONNX time: {onnx_result['avg_time']:.3f}s")

        if speedup > 1.0:
            print(f"  • ONNX is {speedup:.2f}x faster than PyTorch")
        else:
            print(f"  • PyTorch is {1/speedup:.2f}x faster than ONNX")

        print(f"\nReal-time performance:")
        print(f"  • PyTorch: {pytorch_result['speed_x']:.1f}x real-time")
        print(f"  • ONNX: {onnx_result['speed_x']:.1f}x real-time")

    # Transcription quality
    print("\n" + "="*80)
    print("TRANSCRIPTION QUALITY")
    print("="*80)

    transcriptions = [(r['backend'], r['transcription']) for r in results if r and r.get('transcription')]

    if transcriptions:
        for backend, text in transcriptions:
            print(f"\n{backend}:")
            print(f"  {text}")

        # Check if all match
        texts = [t for _, t in transcriptions]
        if len(set(texts)) == 1:
            print("\n✅ All backends produced identical transcriptions")
        else:
            print("\n⚠️  Transcriptions differ between backends")


def main():
    parser = argparse.ArgumentParser(
        description="Benchmark FunASR CPU performance: PyTorch vs ONNX"
    )
    parser.add_argument("--audio", type=str, default="test.wav",
                       help="Path to audio file")
    parser.add_argument("--runs", type=int, default=3,
                       help="Number of runs per backend (default: 3)")
    parser.add_argument("--pytorch-only", action="store_true",
                       help="Test PyTorch backend only")
    parser.add_argument("--onnx-only", action="store_true",
                       help="Test ONNX backend only")
    args = parser.parse_args()

    # Check audio file
    audio_path = Path(args.audio)
    if not audio_path.exists():
        print(f"❌ Audio file not found: {audio_path}")
        sys.exit(1)

    print("="*80)
    print("FunASR CPU Benchmark: PyTorch vs ONNX")
    print("="*80)
    print(f"Audio file: {audio_path}")
    print(f"Number of runs: {args.runs}")

    # Check backend availability
    print("\nChecking backend availability...")
    backends = check_backend_availability()

    if not any(backends.values()):
        print("\n❌ No FunASR backends available!")
        print("\nInstall at least one:")
        print("  pip install funasr         # For PyTorch backend")
        print("  pip install funasr-onnx    # For ONNX backend")
        sys.exit(1)

    results = []

    # Benchmark PyTorch
    if not args.onnx_only and backends['pytorch']:
        result = benchmark_pytorch_cpu(str(audio_path), args.runs)
        results.append(result)

    # Benchmark ONNX
    if not args.pytorch_only and backends['onnx']:
        result = benchmark_onnx_cpu(str(audio_path), args.runs)
        results.append(result)

    # Print comparison
    if results:
        print_comparison(results)

        # Save results
        output_file = "benchmark_cpu_results.json"
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(results, f, indent=2, ensure_ascii=False)
        print(f"\n📊 Results saved to: {output_file}")
    else:
        print("\n❌ No benchmarks were run successfully")


if __name__ == "__main__":
    main()
