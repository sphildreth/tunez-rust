#!/bin/bash

# Example Tunez plugin that implements a simple provider
# This plugin responds to Tunez requests via JSON over stdin/stdout

# Read from stdin and process requests
while IFS= read -r line; do
    # Parse the request ID
    id=$(echo "$line" | grep -o '"id":[0-9]*' | cut -d: -f2)
    
    # Parse the method type
    method=$(echo "$line" | grep -o '"type":"[^"]*"' | cut -d'"' -f4)
    
    case "$method" in
        "Initialize")
            # Respond with plugin info
            echo "{\"id\":$id,\"result\":{\"status\":\"Initialized\",\"id\":\"example-plugin\",\"name\":\"Example Plugin\",\"version\":\"1.0.0\",\"protocol_version\":1}}"
            ;;
        "Capabilities")
            # Respond with plugin capabilities
            echo "{\"id\":$id,\"result\":{\"playlists\":true,\"lyrics\":false,\"artwork\":false,\"favorites\":false,\"recently_played\":false,\"offline_download\":false}}"
            ;;
        "SearchTracks")
            # Respond with example tracks
            echo "{\"id\":$id,\"result\":{\"status\":\"Tracks\",\"items\":[{\"id\":{\"0\":\"example-track-1\"},\"provider_id\":\"example-plugin\",\"title\":\"Example Track 1\",\"artist\":\"Example Artist\",\"album\":\"Example Album\",\"duration_seconds\":180,\"track_number\":1}],\"next\":null}}"
            ;;
        "Shutdown")
            # Acknowledge shutdown
            echo "{\"id\":$id,\"result\":{\"status\":\"ShutdownAck\"}}"
            exit 0
            ;;
        *)
            # For other methods, return a not supported error
            echo "{\"id\":$id,\"result\":{\"status\":\"Error\",\"kind\":\"not_supported\",\"message\":\"Method $method not implemented\"}}"
            ;;
    esac
done