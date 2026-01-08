"""
Fake Conference Controller - Simulates the conference controller for testing
Manages turn-taking and sends control signals to participants
"""

import os
import json
import time
from dora import Node
import pyarrow as pa

def main():
    node = Node()
    
    print("[FAKE_CONTROLLER] Starting fake conference controller")
    
    # State tracking
    current_turn = 0
    turn_order = ["techer", "myself"]  # techer speaks first, then myself
    conversation_started = False
    
    for event in node:
        event_type = event["type"]
        
        if event_type == "INPUT":
            event_id = event["id"]
            data = event["value"]
            
            print(f"[FAKE_CONTROLLER] Received input on port: {event_id}")
            
            if event_id == "control":
                # User control signal (start/stop/next)
                try:
                    control_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_CONTROLLER] Control signal: {control_data}")
                    
                    if "start" in str(control_data).lower():
                        conversation_started = True
                        current_turn = 0
                        
                        # Send initial control to start techer (first speaker)
                        print("[FAKE_CONTROLLER] Starting conversation - techer's turn")
                        node.send_output("control_llm2", pa.array(["start"]))
                        node.send_output("llm_control", pa.array(["active"]))
                        node.send_output("status", pa.array([json.dumps({"status": "started", "turn": "techer"})]))
                        
                    elif "stop" in str(control_data).lower():
                        conversation_started = False
                        node.send_output("llm_control", pa.array(["stop"]))
                        node.send_output("status", pa.array([json.dumps({"status": "stopped"})]))
                        
                except Exception as e:
                    print(f"[FAKE_CONTROLLER] Error parsing control: {e}")
                    
            elif event_id in ["myself", "techer"]:
                # Received text from a participant - advance turn
                if conversation_started:
                    try:
                        text_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                        
                        # Check if this is end of turn (empty or complete marker)
                        if not text_data or text_data == "":
                            current_turn = (current_turn + 1) % len(turn_order)
                            next_speaker = turn_order[current_turn]
                            
                            print(f"[FAKE_CONTROLLER] Turn complete, next speaker: {next_speaker}")
                            
                            # Small delay before next turn
                            time.sleep(0.5)
                            
                            # Send control to next speaker
                            if next_speaker == "techer":
                                node.send_output("control_llm2", pa.array(["speak"]))
                            else:
                                node.send_output("control_llm1", pa.array(["speak"]))
                                
                            node.send_output("status", pa.array([json.dumps({"status": "turn_change", "turn": next_speaker})]))
                            
                    except Exception as e:
                        print(f"[FAKE_CONTROLLER] Error processing participant text: {e}")
                        
            elif event_id == "session_start":
                # Session start signal from audio player
                print("[FAKE_CONTROLLER] Received session_start signal")
                
            elif event_id == "buffer_status":
                # Buffer status from audio player
                try:
                    buffer_data = data[0].as_py() if hasattr(data[0], 'as_py') else str(data[0])
                    print(f"[FAKE_CONTROLLER] Buffer status: {buffer_data}")
                except Exception as e:
                    pass
                    
        elif event_type == "STOP":
            print("[FAKE_CONTROLLER] Stopping...")
            break
        elif event_type == "ERROR":
            print(f"[FAKE_CONTROLLER] Error: {event}")
            
    # Send final log
    node.send_output("log", pa.array(["[FAKE_CONTROLLER] Shutdown complete"]))

if __name__ == "__main__":
    main()
