"""
Fake Conference Bridge - Simulates the bridge that routes messages between participants
Simply forwards messages from one participant to another
"""

import os
import json
from dora import Node
import pyarrow as pa

def main():
    node = Node()
    
    # Get bridge target from environment (who this bridge routes TO)
    bridge_target = os.environ.get("BRIDGE_TARGET", "myself")
    
    print(f"[FAKE_BRIDGE] Starting fake bridge for target: {bridge_target}")
    
    for event in node:
        event_type = event["type"]
        
        if event_type == "INPUT":
            event_id = event["id"]
            data = event["value"]
            
            print(f"[FAKE_BRIDGE][{bridge_target}] Received input on port: {event_id}")
            
            if event_id == "control":
                # Control signal - forward or handle
                try:
                    control_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_BRIDGE][{bridge_target}] Control: {control_data}")
                    
                    if "speak" in str(control_data).lower() or "start" in str(control_data).lower():
                        # Generate initial prompt for the target
                        if bridge_target == "myself":
                            prompt = "请分享你对这个话题的看法。"
                        else:
                            prompt = "请开始今天的讨论。"
                        
                        print(f"[FAKE_BRIDGE][{bridge_target}] Sending prompt: {prompt}")
                        node.send_output("text", pa.array([prompt]))
                        node.send_output("status", pa.array([json.dumps({"status": "sent", "target": bridge_target})]))
                        
                except Exception as e:
                    print(f"[FAKE_BRIDGE][{bridge_target}] Error parsing control: {e}")
                    
            elif event_id in ["myself", "techer"]:
                # Forward message from the other participant
                try:
                    text_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    
                    if text_data:
                        print(f"[FAKE_BRIDGE][{bridge_target}] Forwarding: {text_data[:50]}...")
                        node.send_output("text", pa.array([text_data]))
                        node.send_output("status", pa.array([json.dumps({"status": "forwarded", "target": bridge_target})]))
                        
                except Exception as e:
                    print(f"[FAKE_BRIDGE][{bridge_target}] Error forwarding: {e}")
                    
        elif event_type == "STOP":
            print(f"[FAKE_BRIDGE][{bridge_target}] Stopping...")
            break
        elif event_type == "ERROR":
            print(f"[FAKE_BRIDGE][{bridge_target}] Error: {event}")
            
    node.send_output("log", pa.array([f"[FAKE_BRIDGE][{bridge_target}] Shutdown complete"]))

if __name__ == "__main__":
    main()
