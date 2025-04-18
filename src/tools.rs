use std::collections::HashMap;
use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use macros::function_tool;

pub trait Tool {

    fn metadata(&self) -> ToolMetaData;

    fn execute(&self, parameters: Value) -> anyhow::Result<Value>;
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolMetaData {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl ToolMetaData {
    fn to_tools_call_body(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": self.parameters["properties"],
                    "required": self.parameters["required"],
                }
            }
        })
    }
}

pub trait ToolParameters: for<'de> Deserialize<'de> {
    fn schema() -> Value;
}

#[macro_export]
macro_rules! impl_tool_params {
    ($t:ty) => {
        impl $crate::ToolParameters for $t {
            fn schema() -> Value {
                let schema = schemars::schema_for!($t);
                serde_json::to_value(schema).unwrap()
            }
        }
    }
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = Self {
            tools: HashMap::new(),
        };

        tools.register(AddTool {});
        // tools.register(ExecuteCommandTool {});

        tools
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let metadata = tool.metadata();
        self.tools.insert(metadata.name, Box::new(tool));
    }

    pub fn execute(
        &self,
        tool_name: impl AsRef<str>,
        parameters: Value,
    ) -> anyhow::Result<Value> {
        let res = self.tools
            .get(tool_name.as_ref())
            .expect("Unknown Tool")
            .execute(parameters)?;

        Ok(res)
    }

    pub fn list_metadata(&self) -> Vec<ToolMetaData> {
        self.tools
            .values()
            .map(|t| t.metadata())
            .collect()
    }

    pub fn to_tools_call_body(&self) -> Value {
        serde_json::to_value(
            self.tools
                .iter()
                .map(|(_, item)| item.metadata().to_tools_call_body())
                .collect::<Vec<_>>()
        ).unwrap()
    }
}

struct StubTool;

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StubToolParameters {
    pub message: String,
}

impl_tool_params!(StubToolParameters);

impl Tool for StubTool {
    fn metadata(&self) -> ToolMetaData {
        ToolMetaData {
            name: "stub_tool".to_string(),
            description: "This is an example".to_string(),
            parameters: StubToolParameters::schema(),
        }
    }

    fn execute(&self, parameters: Value) -> anyhow::Result<Value> {
        let params = serde_json::from_value::<StubToolParameters>(parameters)?;
        println!("Execute StubTool {}", params.message);

        Ok(Value::Null)
    }
}

#[function_tool(name = "Add", description = "add a with b")]
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[function_tool(name = "ExecuteCommand", description = "Execute any command you pass by (no check). Return `Ok` if executing successfully, otherwise, return reason.")]
fn execute_command(command: String) -> String {
    todo!() 
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema() {
        let tool = AddTool {};
        let answer = tool.execute(json!({
            "a": 3,
            "b": 5,
        })).unwrap();
        
        println!("{}", serde_json::to_string_pretty(&answer).unwrap());
    }
}
