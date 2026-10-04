use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{Error, Result};

/// A shared application service. Construct providers in settings.rs and receive
/// them in handlers as Inject<T>. T does not have to implement Clone.
pub struct Inject<T>(pub Arc<T>);

impl<T> Inject<T> {
    pub fn into_inner(self) -> Arc<T> {
        self.0
    }
}

impl<T> Clone for Inject<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Deref for Inject<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<S, T> FromRequestParts<S> for Inject<T>
where
    S: Send + Sync,
    T: Send + Sync + 'static,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self> {
        parts
            .extensions
            .get::<Arc<Services>>()
            .and_then(|services| services.values.get(&TypeId::of::<T>()))
            .cloned()
            .and_then(|service| service.downcast::<T>().ok())
            .map(Self)
            .ok_or_else(Error::internal)
    }
}

#[derive(Default)]
pub(crate) struct Services {
    values: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Services {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: Arc<T>) {
        self.values.insert(TypeId::of::<T>(), value);
    }

    pub fn contains(&self, dependency: Dependency) -> bool {
        self.values.contains_key(&dependency.type_id)
    }
}

/// Internal typed dependency metadata produced by route attributes.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Dependency {
    type_id: TypeId,
    pub(crate) name: &'static str,
}

/// Internal compile-time probe. Type aliases resolve normally before lookup.
#[doc(hidden)]
pub struct ServiceProbe<T>(PhantomData<fn() -> T>);

impl<T> Default for ServiceProbe<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Used at concrete handler argument types to select Inject<T> or no dependency.
#[doc(hidden)]
pub trait RequiredService {
    fn required_service(self) -> Option<Dependency>;
}

impl<T> RequiredService for &ServiceProbe<T> {
    fn required_service(self) -> Option<Dependency> {
        None
    }
}

impl<T: Send + Sync + 'static> RequiredService for &&ServiceProbe<Inject<T>> {
    fn required_service(self) -> Option<Dependency> {
        Some(Dependency {
            type_id: TypeId::of::<T>(),
            name: type_name::<T>(),
        })
    }
}
