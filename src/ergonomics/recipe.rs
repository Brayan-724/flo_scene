use crate::context::*;
use crate::entity_id::*;
use crate::error::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use std::sync::*;

///
/// A recipe is used to describe a set of actions sent to one or more entities in a scene, in order.
///
/// This is essentially a simple scripting extension, making it possible to encode fixed sets of steps into
/// a script that can be repeatedly executed (for more complicated scripting, a scripting language should
/// probably be used)
///
/// A recipe is useful in a number of situations, but in particular for testing where it can be used to describe a
/// set of messages and expected responses.
///
#[derive(Clone)]
pub struct Recipe {
    /// Each step is a boxed function returning a future
    steps: Vec<Arc<dyn Send + Fn(Arc<SceneContext>) -> BoxFuture<'static, Result<(), RecipeError>>>>
}

impl Default for Recipe {
    ///
    /// Creates a default (empty) recipe
    ///
    fn default() -> Recipe {
        Recipe {
            steps: vec![]
        }
    }
}

impl Recipe {
    ///
    /// Creates a new empty recipe
    ///
    pub fn new() -> Recipe {
        Self::default()
    }

    ///
    /// Runs this recipe
    ///
    pub async fn run(&self, context: Arc<SceneContext>) -> Result<(), RecipeError> {
        // Run the steps in the recipe, stop if any of them generate an error
        for step in self.steps.iter() {
            step(Arc::clone(&context)).await?;
        }

        Ok(())
    }

    ///
    /// Adds a new step to the recipe that sends a set of fixed messages to an entity
    ///
    pub fn send_messages<TMessage>(self, entity_id: EntityId, messages: impl IntoIterator<Item=TMessage>) -> Recipe
    where
        TMessage: 'static + Clone + Send,
    {
        let mut steps   = self.steps;
        let messages    = messages.into_iter().collect::<Vec<_>>();
        let new_step    = Arc::new(move |context: Arc<SceneContext>| {
            let messages = messages.clone();

            async move {
                // Send to the entity
                let mut channel = context.send_to(entity_id)?;

                // Copy the messages one at a time
                for msg in messages.into_iter() {
                    channel.send(msg).await?;
                }

                Ok(())
            }.boxed()
        });

        steps.push(new_step);

        Recipe { steps }
    }
}
