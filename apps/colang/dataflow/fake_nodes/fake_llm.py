"""
Fake LLM Node - Simulates AI API responses for testing
Generates predefined responses to simulate conversation flow
"""

import os
import json
import time
from dora import Node
import pyarrow as pa

# Predefined responses for simulation
FAKE_RESPONSES = {
    "myself": [
        "你好！我是学生小明，很高兴参加今天的讨论。",
        "我认为这个话题非常有趣。让我分享一下我的观点。",
        "从我的角度来看，这个问题有很多方面需要考虑。",
        "我同意老师的看法，这确实是一个重要的问题。",
        "让我补充一点，我觉得我们还需要考虑实际应用场景。",
    ],
    "techer": [
        "同学们好！今天我们来讨论一个有趣的话题。",
        "很好的问题！让我从专业角度来解释一下。",
        "这个观点很有见地。我来补充一些背景知识。",
        "大家说得都很好。让我总结一下关键要点。",
        "非常精彩的讨论！希望大家继续保持学习热情。",
    ],
}

def main():
    node = Node()
    
    # Get participant ID from environment (myself or techer)
    participant_id = os.environ.get("PARTICIPANT_ID", "myself")
    responses = FAKE_RESPONSES.get(participant_id, FAKE_RESPONSES["myself"])
    response_index = 0
    
    print(f"[FAKE_LLM] Starting fake LLM node for participant: {participant_id}")
    
    for event in node:
        event_type = event["type"]
        
        if event_type == "INPUT":
            event_id = event["id"]
            data = event["value"]
            
            print(f"[FAKE_LLM][{participant_id}] Received input on port: {event_id}")
            
            if event_id == "text":
                # Received input text, generate fake response
                try:
                    input_text = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_LLM][{participant_id}] Input: {input_text[:100]}...")
                except Exception as e:
                    print(f"[FAKE_LLM][{participant_id}] Error parsing input: {e}")
                    input_text = ""
                
                # Generate response with slight delay to simulate API call
                time.sleep(0.5)
                
                response = responses[response_index % len(responses)]
                response_index += 1
                
                print(f"[FAKE_LLM][{participant_id}] Sending response: {response[:50]}...")
                
                # Send response as text output (streaming simulation)
                for i, char in enumerate(response):
                    # Send character by character to simulate streaming
                    node.send_output("text", pa.array([char]), {"streaming": True, "index": i})
                    time.sleep(0.02)  # 20ms delay between characters
                
                # Send end of stream marker
                node.send_output("text", pa.array([""]), {"streaming": False, "complete": True})
                
                # Send status
                status_msg = json.dumps({"status": "complete", "participant": participant_id})
                node.send_output("status", pa.array([status_msg]))
                
            elif event_id == "control":
                # Handle control signals
                try:
                    control_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_LLM][{participant_id}] Control signal: {control_data}")
                    
                    if "reset" in str(control_data).lower():
                        response_index = 0
                        print(f"[FAKE_LLM][{participant_id}] Reset response index")
                except Exception as e:
                    print(f"[FAKE_LLM][{participant_id}] Error parsing control: {e}")
                    
        elif event_type == "STOP":
            print(f"[FAKE_LLM][{participant_id}] Stopping...")
            break
        elif event_type == "ERROR":
            print(f"[FAKE_LLM][{participant_id}] Error: {event}")

if __name__ == "__main__":
    main()
