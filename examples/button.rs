use fyrox::event_loop::EventLoop;
use fyrox::{
    core::{algebra::Vector2, pool::Handle, rand::Rng},
    engine::{executor::Executor, GraphicsContext, GraphicsContextParams},
    event_loop::ControlFlow,
    gui::{
        button::{ButtonBuilder, ButtonMessage},
        message::{MessageDirection, UiMessage},
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
    plugin::{Plugin, PluginConstructor, PluginContext},
    rand::thread_rng,
    scene::Scene,
    window::WindowAttributes,
};

struct Game {
    button: Handle<UiNode>,
}

impl Plugin for Game {
    fn on_ui_message(
        &mut self,
        context: &mut PluginContext,
        message: &UiMessage,
        _control_flow: &mut ControlFlow,
    ) {
        // Simple example of message system. We'll catch "Click" messages from the button
        // and send new message to the button that will contain new position for it.
        if let Some(ButtonMessage::Click) = message.data::<ButtonMessage>() {
            if message.destination() == self.button {
                // Generate random position in the window.
                if let GraphicsContext::Initialized(ref graphics_context) = context.graphics_context
                {
                    let client_size = graphics_context.window.inner_size();

                    let mut rng = thread_rng();

                    let new_position = Vector2::new(
                        rng.gen_range(0.0..(client_size.width as f32 - 100.0)),
                        rng.gen_range(0.0..(client_size.height as f32 - 100.0)),
                    );

                    // "Tell" the button to "teleport" in the new location.
                    context
                        .user_interface
                        .send_message(WidgetMessage::desired_position(
                            self.button,
                            MessageDirection::ToWidget,
                            new_position,
                        ));
                }
            }
        }
    }
}

struct GameConstructor;

impl PluginConstructor for GameConstructor {
    fn create_instance(
        &self,
        _override_scene: Handle<Scene>,
        context: PluginContext,
    ) -> Box<dyn Plugin> {
        let ctx = &mut context.user_interface.build_ctx();

        // The simplest button can be created in a few lines of code.
        let button = ButtonBuilder::new(WidgetBuilder::new())
            .with_text("Click me!")
            .build(ctx);

        Box::new(Game { button })
    }
}

fn main() {
    let mut executor = Executor::from_params(
        EventLoop::new().unwrap(),
        GraphicsContextParams {
            window_attributes: WindowAttributes {
                title: "Example - Button".to_string(),
                ..Default::default()
            },
            vsync: true,
        },
    );
    executor.add_plugin_constructor(GameConstructor);
    executor.run()
}
