"""
Fake TTS Node - Simulates Text-to-Speech conversion
Generates fake audio data for testing the audio pipeline
"""

import os
import json
import time
import struct
from dora import Node
import pyarrow as pa

def generate_fake_audio(text, sample_rate=22050, duration_per_char=0.05):
    """
    Generate fake audio data (silence with proper WAV-like structure)
    Returns bytes that can be processed by audio player
    """
    # Calculate duration based on text length
    duration = len(text) * duration_per_char
    num_samples = int(sample_rate * duration)
    
    # Generate silence (zeros) as fake audio samples
    # Using 16-bit signed integers
    audio_samples = [0] * num_samples
    
    # Pack as 16-bit signed integers (little-endian)
    audio_bytes = struct.pack(f'<{num_samples}h', *audio_samples)
    
    return audio_bytes, sample_rate

def main():
    node = Node()
    
    # Get voice name from environment
    voice_name = os.environ.get("VOICE_NAME", "Default")
    participant_id = os.environ.get("PARTICIPANT_ID", "unknown")
    
    print(f"[FAKE_TTS] Starting fake TTS node - Voice: {voice_name}, Participant: {participant_id}")
    
    for event in node:
        event_type = event["type"]
        
        if event_type == "INPUT":
            event_id = event["id"]
            data = event["value"]
            
            print(f"[FAKE_TTS][{voice_name}] Received input on port: {event_id}")
            
            if event_id == "text":
                try:
                    text_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    
                    if text_data:
                        print(f"[FAKE_TTS][{voice_name}] Synthesizing: {text_data[:50]}...")
                        
                        # Simulate TTS processing time
                        process_time = len(text_data) * 0.01  # 10ms per character
                        time.sleep(min(process_time, 1.0))  # Cap at 1 second
                        
                        # Generate fake audio
                        audio_bytes, sample_rate = generate_fake_audio(text_data)
                        
                        # Create audio metadata
                        audio_meta = {
                            "sample_rate": sample_rate,
                            "channels": 1,
                            "format": "s16le",  # 16-bit signed little-endian
                            "text": text_data,
                            "voice": voice_name,
                            "participant": participant_id,
                        }
                        
                        print(f"[FAKE_TTS][{voice_name}] Sending audio: {len(audio_bytes)} bytes")
                        
                        # Send audio output
                        node.send_output("audio", pa.array([audio_bytes]), audio_meta)
                        
                        # Send status
                        node.send_output("status", pa.array([json.dumps({
                            "status": "synthesized",
                            "text_length": len(text_data),
                            "audio_bytes": len(audio_bytes),
                            "voice": voice_name,
                        })]))
                        
                        # Send segment complete signal
                        node.send_output("segment_complete", pa.array([json.dumps({
                            "text": text_data,
                            "voice": voice_name,
                        })]))
                        
                except Exception as e:
                    print(f"[FAKE_TTS][{voice_name}] Error: {e}")
                    node.send_output("status", pa.array([json.dumps({"status": "error", "error": str(e)})]))
                    
        elif event_type == "STOP":
            print(f"[FAKE_TTS][{voice_name}] Stopping...")
            break
        elif event_type == "ERROR":
            print(f"[FAKE_TTS][{voice_name}] Error: {event}")
            
    node.send_output("log", pa.array([f"[FAKE_TTS][{voice_name}] Shutdown complete"]))

if __name__ == "__main__":
    main()
