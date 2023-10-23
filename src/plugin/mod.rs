//! Everything related to plugins. See [`Plugin`] docs for more info.

#![warn(missing_docs)]

use crate::{
    asset::manager::ResourceManager,
    core::pool::Handle,
    engine::{
        AsyncSceneLoader, GraphicsContext, PerformanceStatistics, ScriptProcessor,
        SerializationContext,
    },
    event::Event,
    gui::{message::UiMessage, UserInterface},
    scene::{Scene, SceneContainer},
};
use fyrox_core::visitor::VisitError;
use std::{any::Any, path::Path, sync::Arc};
use winit::event_loop::EventLoopWindowTarget;

/// Plugin constructor is a first step of 2-stage plugin initialization. It is responsible for plugin script
/// registration and for creating actual plugin instance.
///
/// # Details
///
/// Why there is a need in 2-state initialization? The editor requires it, it is interested only in plugin
/// scripts so editor does not create any plugin instances, it just uses [Self::register] to obtain information
/// about scripts.  
pub trait PluginConstructor {
    /// The method is called when the plugin constructor was just registered in the engine. The main use of the
    /// method is to register scripts and custom scene graph nodes in [`SerializationContext`].
    fn register(&self, #[allow(unused_variables)] context: PluginRegistrationContext) {}

    /// The method is called when the engine creates plugin instances. It allows to create initialized plugin
    /// instance.
    ///
    /// ## Arguments
    ///
    /// `scene_path` argument tells you that there's already a scene specified. It is used primarily
    /// by the editor, to run your game with a scene you have current opened in the editor. Typical
    /// usage would be: `scene_path.unwrap_or("a/path/to/my/default/scene.rgs")`
    fn create_instance(&self, scene_path: Option<&str>, context: PluginContext) -> Box<dyn Plugin>;
}

/// Contains plugin environment for the registration stage.
pub struct PluginRegistrationContext<'a> {
    /// A reference to serialization context of the engine. See [`SerializationContext`] for more
    /// info.
    pub serialization_context: &'a Arc<SerializationContext>,
    /// A reference to the resource manager instance of the engine. Could be used to register resource loaders.
    pub resource_manager: &'a ResourceManager,
}

/// Contains plugin environment.
pub struct PluginContext<'a, 'b> {
    /// A reference to scene container of the engine. You can add new scenes from [`Plugin`] methods
    /// by using [`SceneContainer::add`].
    pub scenes: &'a mut SceneContainer,

    /// A reference to the resource manager, it can be used to load various resources and manage
    /// them. See [`ResourceManager`] docs for more info.
    pub resource_manager: &'a ResourceManager,

    /// A reference to user interface instance.
    pub user_interface: &'a mut UserInterface,

    /// A reference to the graphics_context, it contains a reference to the window and the current renderer.
    /// It could be [`GraphicsContext::Uninitialized`] if your application is suspended (possible only on
    /// Android; it is safe to call [`GraphicsContext::as_initialized_ref`] or [`GraphicsContext::as_initialized_mut`]
    /// on every other platform).
    pub graphics_context: &'a mut GraphicsContext,

    /// The time (in seconds) that passed since last call of a method in which the context was
    /// passed. It has fixed value that is defined by a caller (in most cases it is `Executor`).
    pub dt: f32,

    /// A reference to time accumulator, that holds remaining amount of time that should be used
    /// to update a plugin. A caller splits `lag` into multiple sub-steps using `dt` and thus
    /// stabilizes update rate. The main use of this variable, is to be able to reset `lag` when
    /// you doing some heavy calculations in a your game loop (i.e. loading a new level) so the
    /// engine won't try to "catch up" with all the time that was spent in heavy calculation.
    pub lag: &'b mut f32,

    /// A reference to serialization context of the engine. See [`SerializationContext`] for more
    /// info.
    pub serialization_context: &'a Arc<SerializationContext>,

    /// Performance statistics from the last frame.
    pub performance_statistics: &'a PerformanceStatistics,

    /// Amount of time (in seconds) that passed from creation of the engine. Keep in mind, that
    /// this value is **not** guaranteed to match real time. A user can change delta time with
    /// which the engine "ticks" and this delta time affects elapsed time.
    pub elapsed_time: f32,

    /// Script processor is used to run script methods in a strict order.
    pub script_processor: &'a ScriptProcessor,

    /// Asynchronous scene loader. It is used to request scene loading. See [`AsyncSceneLoader`] docs
    /// for usage example.
    pub async_scene_loader: &'a mut AsyncSceneLoader,

    /// Special field that associates main application event loop (not game loop) with OS-specific
    /// windows. It also can be used to alternate control flow of the application.
    pub window_target: Option<&'b EventLoopWindowTarget<()>>,
}

/// Base plugin automatically implements type casting for plugins.
pub trait BasePlugin: Any + 'static {
    /// Returns a reference to Any trait. It is used for type casting.
    fn as_any(&self) -> &dyn Any;

    /// Returns a reference to Any trait. It is used for type casting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> BasePlugin for T
where
    T: Any + Plugin + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl dyn Plugin {
    /// Performs downcasting to a particular type.
    pub fn cast<T: Plugin>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    /// Performs downcasting to a particular type.
    pub fn cast_mut<T: Plugin>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

/// Plugin is a convenient interface that allow you to extend engine's functionality.
///
/// # Static vs dynamic plugins
///
/// Every plugin must be linked statically to ensure that everything is memory safe. There was some
/// long research about hot reloading and dynamic plugins (in DLLs) and it turned out that they're
/// not guaranteed to be memory safe because Rust does not have stable ABI. When a plugin compiled
/// into DLL, Rust compiler is free to reorder struct members in any way it needs to. It is not
/// guaranteed that two projects that uses the same library will have compatible ABI. This fact
/// indicates that you either have to use static linking of your plugins or provide C interface
/// to every part of the engine and "communicate" with plugin using C interface with C ABI (which
/// is standardized and guaranteed to be compatible). The main problem with C interface is
/// boilerplate code and the need to mark every structure "visible" through C interface with
/// `#[repr(C)]` attribute which is not always easy and even possible (because some structures could
/// be re-exported from dependencies). These are the main reasons why the engine uses static plugins.
///
/// # Example
///
/// ```rust
/// use fyrox::{
///     core::{pool::Handle},
///     plugin::{Plugin, PluginContext, PluginRegistrationContext},
///     scene::Scene,
///     event::Event
/// };
/// use std::str::FromStr;
///
/// #[derive(Default)]
/// struct MyPlugin {}
///
/// impl Plugin for MyPlugin {
///     fn on_deinit(&mut self, context: PluginContext) {
///         // The method is called when the plugin is disabling.
///         // The implementation is optional.
///     }
///
///     fn update(&mut self, context: &mut PluginContext) {
///         // The method is called on every frame, it is guaranteed to have fixed update rate.
///         // The implementation is optional.
///     }
///
///     fn on_os_event(&mut self, event: &Event<()>, context: PluginContext) {
///         // The method is called when the main window receives an event from the OS.
///     }
/// }
/// ```
pub trait Plugin: BasePlugin {
    /// The method is called before plugin will be disabled. It should be used for clean up, or some
    /// additional actions.
    fn on_deinit(&mut self, #[allow(unused_variables)] context: PluginContext) {}

    /// Updates the plugin internals at fixed rate (see [`PluginContext::dt`] parameter for more
    /// info).
    fn update(&mut self, #[allow(unused_variables)] context: &mut PluginContext) {}

    /// The method is called when the main window receives an event from the OS. The main use of
    /// the method is to respond to some external events, for example an event from keyboard or
    /// gamepad. See [`Event`] docs for more info.
    fn on_os_event(
        &mut self,
        #[allow(unused_variables)] event: &Event<()>,
        #[allow(unused_variables)] context: PluginContext,
    ) {
    }

    /// The method is called when a graphics context was successfully created. It could be useful
    /// to catch the moment when it was just created and do something in response.
    fn on_graphics_context_initialized(
        &mut self,
        #[allow(unused_variables)] context: PluginContext,
    ) {
    }

    /// The method is called before the actual frame rendering. It could be useful to render off-screen
    /// data (render something to texture, that can be used later in the main frame).
    fn before_rendering(&mut self, #[allow(unused_variables)] context: PluginContext) {}

    /// The method is called when the current graphics context was destroyed.
    fn on_graphics_context_destroyed(&mut self, #[allow(unused_variables)] context: PluginContext) {
    }

    /// The method will be called when there is any message from main user interface instance
    /// of the engine.
    fn on_ui_message(
        &mut self,
        #[allow(unused_variables)] context: &mut PluginContext,
        #[allow(unused_variables)] message: &UiMessage,
    ) {
    }

    /// This method is called when the engine starts loading a scene from the given `path`. It could
    /// be used to "catch" the moment when the scene is about to be loaded; to show a progress bar
    /// for example. See [`AsyncSceneLoader`] docs for usage example.
    fn on_scene_begin_loading(
        &mut self,
        #[allow(unused_variables)] path: &Path,
        #[allow(unused_variables)] context: &mut PluginContext,
    ) {
    }

    /// This method is called when the engine finishes loading a scene from the given `path`. Use
    /// this method if you need do something with a newly loaded scene. See [`AsyncSceneLoader`] docs
    /// for usage example.
    fn on_scene_loaded(
        &mut self,
        #[allow(unused_variables)] path: &Path,
        #[allow(unused_variables)] scene: Handle<Scene>,
        #[allow(unused_variables)] data: &[u8],
        #[allow(unused_variables)] context: &mut PluginContext,
    ) {
    }

    /// This method is called when the engine finishes loading a scene from the given `path` with
    /// some error. This method could be used to report any issues to a user.
    fn on_scene_loading_failed(
        &mut self,
        #[allow(unused_variables)] path: &Path,
        #[allow(unused_variables)] error: &VisitError,
        #[allow(unused_variables)] context: &mut PluginContext,
    ) {
    }
}
