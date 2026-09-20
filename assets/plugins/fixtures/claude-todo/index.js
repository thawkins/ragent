// Claude-dialect fixture plugin (spec `plugins` acceptance criteria 1 and 4).
//
// The tool and command are declared in the manifest (so they list while the
// plugin is disabled); their JavaScript handlers are registered at load time.
// The lifecycle merges the manifest declaration with the runtime registration
// by name, keeping the manifest metadata and the runtime handler.

ragent.register_tool({
  name: "add_todo",
  description: "Add an item to the plugin's todo list",
  parameters: {
    type: "object",
    properties: {
      text: { type: "string", description: "Todo item text" }
    },
    required: ["text"]
  },
  handler: function (args) {
    var text = args && args.text ? String(args.text) : "";
    return { added: true, text: text };
  }
});

ragent.register_command({
  name: "todo-add",
  description: "Add a todo item",
  usage: "/todo-add <text>",
  handler: function (text) {
    return "Added todo: " + text;
  }
});
