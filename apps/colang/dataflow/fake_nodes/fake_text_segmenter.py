"""
Fake Text Segmenter - Simulates text segmentation for TTS
Segments incoming text into sentences for TTS processing
"""

import os
import json
import re
from dora import Node
import pyarrow as pa

# Punctuation marks for sentence splitting
PUNCTUATION = "。！？.!?，,；"

def segment_text(text):
    """Split text into sentences at punctuation marks"""
    if not text:
        return []
    
    # Use regex to split at punctuation while keeping the punctuation
    segments = re.split(f'([{re.escape(PUNCTUATION)}])', text)
    
    # Combine text with its following punctuation
    result = []
    current = ""
    for s in segments:
        if s in PUNCTUATION:
            current += s
            if current.strip():
                result.append(current.strip())
            current = ""
        else:
            current += s
            
    if current.strip():
        result.append(current.strip())
        
    return result

def main():
    node = Node()
    
    print("[FAKE_SEGMENTER] Starting fake text segmenter")
    
    # Buffer for accumulating streaming text
    buffers = {"myself": "", "techer": ""}
    
    for event in node:
        event_type = event["type"]
        
        if event_type == "INPUT":
            event_id = event["id"]
            data = event["value"]
            metadata = event.get("metadata", {})
            
            print(f"[FAKE_SEGMENTER] Received input on port: {event_id}")
            
            if event_id in ["myself", "techer"]:
                # Text from participant
                try:
                    text_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    
                    # Check if streaming
                    is_streaming = metadata.get("streaming", False) if metadata else False
                    is_complete = metadata.get("complete", False) if metadata else False
                    
                    if is_streaming and not is_complete:
                        # Accumulate streaming text
                        buffers[event_id] += text_data
                        
                        # Try to extract complete sentences
                        segments = segment_text(buffers[event_id])
                        if len(segments) > 1:
                            # Send all complete segments except the last (incomplete) one
                            for seg in segments[:-1]:
                                output_port = f"text_segment_{event_id}"
                                print(f"[FAKE_SEGMENTER] Sending segment to {output_port}: {seg}")
                                node.send_output(output_port, pa.array([seg]))
                            
                            # Keep the last incomplete segment in buffer
                            buffers[event_id] = segments[-1]
                    else:
                        # Complete message or non-streaming - send remaining buffer
                        if buffers[event_id]:
                            remaining = buffers[event_id] + text_data
                            segments = segment_text(remaining)
                            for seg in segments:
                                output_port = f"text_segment_{event_id}"
                                print(f"[FAKE_SEGMENTER] Sending final segment to {output_port}: {seg}")
                                node.send_output(output_port, pa.array([seg]))
                            buffers[event_id] = ""
                        elif text_data:
                            segments = segment_text(text_data)
                            for seg in segments:
                                output_port = f"text_segment_{event_id}"
                                print(f"[FAKE_SEGMENTER] Sending segment to {output_port}: {seg}")
                                node.send_output(output_port, pa.array([seg]))
                                
                    node.send_output("status", pa.array([json.dumps({"status": "processed", "source": event_id})]))
                    
                except Exception as e:
                    print(f"[FAKE_SEGMENTER] Error processing text: {e}")
                    
            elif event_id == "audio_complete":
                # Audio playback complete signal
                print("[FAKE_SEGMENTER] Received audio_complete signal")
                node.send_output("status", pa.array([json.dumps({"status": "audio_complete"})]))
                
            elif event_id == "audio_buffer_control":
                # Buffer control signal
                print("[FAKE_SEGMENTER] Received buffer control signal")
                
            elif event_id in ["control", "reset"]:
                # Control signals
                try:
                    control_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_SEGMENTER] Control signal: {control_data}")
                    
                    if "reset" in str(control_data).lower():
                        buffers = {"myself": "", "techer": ""}
                        print("[FAKE_SEGMENTER] Buffers reset")
                except Exception as e:
                    print(f"[FAKE_SEGMENTER] Error parsing control: {e}")
                    
        elif event_type == "STOP":
            print("[FAKE_SEGMENTER] Stopping...")
            break
        elif event_type == "ERROR":
            print(f"[FAKE_SEGMENTER] Error: {event}")
            
    node.send_output("log", pa.array(["[FAKE_SEGMENTER] Shutdown complete"]))

if __name__ == "__main__":
    main()
