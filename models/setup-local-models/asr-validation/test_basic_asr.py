#!/usr/bin/env python3
"""
Basic ASR test script - tests transcription with the provided audio file
"""

import os
import sys
import time
import librosa
from pathlib import Path

# Add dora-asr to path if needed
asr_path = Path(__file__).parent.parent.parent.parent / "python-nodes" / "dora-asr"
if asr_path.exists():
    sys.path.insert(0, str(asr_path))

def test_basic_asr():
    """Test basic ASR functionality"""
    
    # Check USE_GPU environment variable
    use_gpu = os.getenv("USE_GPU", "false").lower() == "true"
    
    print("="*60)
    print(f"Basic ASR Test (USE_GPU={use_gpu})")
    print("="*60)
    
    # Import ASR manager
    from dora_asr.manager import ASRManager
    from dora_asr.config import ASRConfig
    
    # Check configuration
    config = ASRConfig()
    print(f"\nConfiguration:")
    print(f"  USE_GPU: {config.USE_GPU}")
    print(f"  ASR_ENGINE: {config.ASR_ENGINE}")
    print(f"  LANGUAGE: {config.LANGUAGE}")
    print(f"  Models dir: {config.get_models_dir()}")
    
    # Initialize manager
    print("\nInitializing ASR manager...")
    manager = ASRManager()
    
    # Load test audio
    audio_file = "test.wav"
    if not Path(audio_file).exists():
        print(f"Error: Audio file '{audio_file}' not found!")
        print("Please run this script from the asr-validation directory")
        return 1
    
    print(f"\nLoading audio: {audio_file}")
    audio_data, sr = librosa.load(audio_file, sr=16000)
    duration = len(audio_data) / sr
    print(f"  Duration: {duration:.2f} seconds")
    print(f"  Sample rate: {sr} Hz")
    
    # Perform transcription
    print("\nPerforming transcription...")
    start_time = time.time()
    
    try:
        result = manager.transcribe(audio_data, language='zh')
        elapsed = time.time() - start_time
        
        # Display results
        print(f"\n{'='*60}")
        print("RESULTS")
        print(f"{'='*60}")
        print(f"Processing time: {elapsed:.3f} seconds")
        print(f"Real-time factor: {elapsed/duration:.3f}x")
        print(f"Speed: {duration/elapsed:.1f}x real-time")
        print(f"\nTranscription:")
        print(f"  {result.get('text', 'No text returned')}")
        print(f"\nLanguage: {result.get('language', 'Unknown')}")
        
        # Check which engine was used
        if 'funasr' in manager._engines:
            engine = manager._engines['funasr']
            print(f"\nEngine details:")
            print(f"  Class: {engine.__class__.__name__}")
            if hasattr(engine, 'device'):
                print(f"  Device: {engine.device}")
            if hasattr(engine, 'use_pytorch'):
                print(f"  Backend: {'PyTorch' if engine.use_pytorch else 'ONNX'}")
        
        # Validate transcription
        expected_text = "目前的等级为二等站。"
        if result.get('text'):
            # Simple similarity check (you might want more sophisticated comparison)
            if "你好吗" in result['text'] and "北京动物园" in result['text']:
                print(f"\n✅ Transcription quality: GOOD (contains expected phrases)")
            else:
                print(f"\n⚠️ Transcription quality: Check manually")
                print(f"Expected: {expected_text}")
        
        print(f"\n{'='*60}")
        print("Test completed successfully!")
        print(f"{'='*60}")
        
    except Exception as e:
        print(f"\n❌ Transcription failed: {e}")
        import traceback
        traceback.print_exc()
        return 1
    
    finally:
        # Cleanup
        manager.cleanup()
    
    return 0

if __name__ == "__main__":
    sys.exit(test_basic_asr())
