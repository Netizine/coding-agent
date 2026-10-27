// Development-only response oracle. No model provider, credentials, or network calls.
package main

import (
	"encoding/json"
	"fmt"
	"github.com/microsoft/agent-framework-go/agent"
	"github.com/microsoft/agent-framework-go/message"
	"os"
	"time"
)

type Case struct {
	Name     string                  `json:"name"`
	Input    message.Contents        `json:"input"`
	Updates  []*agent.ResponseUpdate `json:"updates"`
	Expected json.RawMessage         `json:"expected"`
}

func main() {
	if len(os.Args) < 2 || len(os.Args) > 3 {
		panic("usage: go run . FIXTURES [content]")
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		panic(err)
	}
	var cases []Case
	if err := json.Unmarshal(data, &cases); err != nil {
		panic(err)
	}
	for _, fixture := range cases {
		var response agent.Response
		for _, update := range fixture.Updates {
			response.Update(update)
		}
		response.Coalesce()
		ids := []string{}
		for _, message := range response.Messages {
			ids = append(ids, message.ID)
		}
		var created any
		if response.CreatedAt.After(time.Unix(0, 0)) {
			created = response.CreatedAt.Format("2006-01-02T15:04:05-07:00")
		}
		usage := response.Usage()
		actual := map[string]any{"text": response.String(), "message_ids": ids, "finish_reason": response.FinishReason,
			"continuation_token": response.ContinuationToken, "conversation_id": response.ConversationID,
			"input_tokens": usage.InputTokenCount, "output_tokens": usage.OutputTokenCount, "created_at": created}
		var result any = actual
		if len(os.Args) == 3 && os.Args[2] == "content" {
			result = fixture.Input.Coalesce()
		}
		actualJSON, err := json.Marshal(result)
		if err != nil {
			panic(err)
		}
		var actualValue, expectedValue any
		if err := json.Unmarshal(actualJSON, &actualValue); err != nil {
			panic(err)
		}
		if err := json.Unmarshal(fixture.Expected, &expectedValue); err != nil {
			panic(err)
		}
		actualJSON, _ = json.Marshal(actualValue)
		expectedJSON, _ := json.Marshal(expectedValue)
		if string(actualJSON) != string(expectedJSON) {
			panic(fmt.Sprintf("%s: got %s; expected %s", fixture.Name, actualJSON, expectedJSON))
		}
		fmt.Println("PASS", fixture.Name)
	}
}
