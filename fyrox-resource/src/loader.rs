//! Resource loader. It manages resource loading.

use crate::{event::ResourceEventBroadcaster, UntypedResource};
use fyrox_core::uuid::Uuid;
use std::{any::Any, future::Future, pin::Pin};

#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub trait BaseResourceLoader: 'static {}

#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub trait BaseResourceLoader: Send + 'static {}

impl<T> BaseResourceLoader for T where T: ResourceLoader {}

/// A simple type-casting trait that has auto-impl.
pub trait ResourceLoaderTypeTrait: BaseResourceLoader {
    /// Converts `self` into boxed `Any`.
    fn into_any(self: Box<Self>) -> Box<dyn Any>;

    /// Returns `self` as `&dyn Any`. It is useful for downcasting to a particular type.
    fn as_any(&self) -> &dyn Any;

    /// Returns `self` as `&mut dyn Any`. It is useful for downcasting to a particular type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> ResourceLoaderTypeTrait for T
where
    T: ResourceLoader,
{
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Trait for resource loading.
pub trait ResourceLoader: ResourceLoaderTypeTrait {
    /// Returns a list of file extensions supported by the loader. Resource manager will use this list
    /// to pick the correct resource loader when the user requests a resource.
    fn extensions(&self) -> &[&str];

    /// Must return a type uuid of the resource data type.
    fn data_type_uuid(&self) -> Uuid;

    /// Loads or reloads a resource.
    fn load(
        &self,
        resource: UntypedResource,
        event_broadcaster: ResourceEventBroadcaster,
        reload: bool,
    ) -> BoxedLoaderFuture;
}

/// Future type for resource loading. See 'ResourceLoader'.
#[cfg(target_arch = "wasm32")]
pub type BoxedLoaderFuture = Pin<Box<dyn Future<Output = ()>>>;

/// Future type for resource loading. See 'ResourceLoader'.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxedLoaderFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Container for resource loaders.
#[derive(Default)]
pub struct ResourceLoadersContainer {
    loaders: Vec<Box<dyn ResourceLoader>>,
}

impl ResourceLoadersContainer {
    /// Creates new empty resource loaders container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds new resource loader or replaces existing. There could be only one loader of a given type
    /// at the same time. You can use this method to replace resource loaders with your own loaders.
    pub fn set<T>(&mut self, loader: T) -> Option<T>
    where
        T: ResourceLoader,
    {
        if let Some(existing_loader) = self
            .loaders
            .iter_mut()
            .find_map(|l| (**l).as_any_mut().downcast_mut::<T>())
        {
            Some(std::mem::replace(existing_loader, loader))
        } else {
            self.loaders.push(Box::new(loader));
            None
        }
    }

    /// Searches for an instance of a resource loader of type `Prev` and replaces it with an other instance
    /// of a type `New`.
    pub fn try_replace<Prev, New>(&mut self, new_loader: New) -> Option<Prev>
    where
        Prev: ResourceLoader,
        New: ResourceLoader,
    {
        if let Some(pos) = self
            .loaders
            .iter()
            .position(|l| (**l).as_any().is::<Prev>())
        {
            let prev_untyped = std::mem::replace(&mut self.loaders[pos], Box::new(new_loader));
            prev_untyped
                .into_any()
                .downcast::<Prev>()
                .ok()
                .map(|boxed| *boxed)
        } else {
            None
        }
    }

    /// Tries to find an instance of a resource loader of the given type `T.
    pub fn find<T>(&self) -> Option<&T>
    where
        T: ResourceLoader,
    {
        self.loaders
            .iter()
            .find_map(|loader| (**loader).as_any().downcast_ref())
    }

    /// Tries to find an instance of a resource loader of the given type `T.
    pub fn find_mut<T>(&mut self) -> Option<&mut T>
    where
        T: ResourceLoader,
    {
        self.loaders
            .iter_mut()
            .find_map(|loader| (**loader).as_any_mut().downcast_mut())
    }

    /// Returns total amount of resource loaders in the container.
    pub fn len(&self) -> usize {
        self.loaders.len()
    }

    /// Return `true` if the container contains no resource loaders.
    pub fn is_empty(&self) -> bool {
        self.loaders.is_empty()
    }

    /// Returns an iterator yielding shared references to "untyped" resource loaders.
    pub fn iter(&self) -> impl Iterator<Item = &dyn ResourceLoader> {
        self.loaders.iter().map(|boxed| &**boxed)
    }

    /// Returns an iterator yielding mutable references to "untyped" resource loaders.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut dyn ResourceLoader> {
        self.loaders.iter_mut().map(|boxed| &mut **boxed)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Eq, PartialEq, Debug)]
    struct MyResourceLoader;

    impl ResourceLoader for MyResourceLoader {
        fn extensions(&self) -> &[&str] {
            &[]
        }

        fn data_type_uuid(&self) -> Uuid {
            Default::default()
        }

        fn load(
            &self,
            _resource: UntypedResource,
            _event_broadcaster: ResourceEventBroadcaster,
            _reload: bool,
        ) -> BoxedLoaderFuture {
            todo!()
        }
    }

    #[test]
    fn resource_loader_container_new() {
        let container = ResourceLoadersContainer::new();
        assert!(container.loaders.is_empty());

        let container = ResourceLoadersContainer::default();
        assert!(container.loaders.is_empty());
    }

    #[test]
    fn resource_loader_container_set() {
        let mut container = ResourceLoadersContainer::new();
        let res = container.set(MyResourceLoader);
        let res2 = container.set(MyResourceLoader);
        assert_eq!(res, None);
        assert_eq!(res2, Some(MyResourceLoader));

        assert_eq!(container.len(), 1);
    }

    #[test]
    fn resource_loader_container_find() {
        let mut container = ResourceLoadersContainer::new();

        let res = container.find::<MyResourceLoader>();
        assert_eq!(res, None);

        container.set(MyResourceLoader);
        let res = container.find::<MyResourceLoader>();

        assert_eq!(res, Some(&MyResourceLoader));
    }

    #[test]
    fn resource_loader_container_find_mut() {
        let mut container = ResourceLoadersContainer::new();

        let res = container.find_mut::<MyResourceLoader>();
        assert_eq!(res, None);

        container.set(MyResourceLoader);
        let res = container.find_mut::<MyResourceLoader>();

        assert_eq!(res, Some(&mut MyResourceLoader));
    }

    #[test]
    fn resource_loader_container_getters() {
        let mut container = ResourceLoadersContainer::new();
        assert!(container.is_empty());
        assert_eq!(container.len(), 0);

        container.set(MyResourceLoader);
        container.set(MyResourceLoader);
        assert!(!container.is_empty());
        assert_eq!(container.len(), 1);
    }
}
