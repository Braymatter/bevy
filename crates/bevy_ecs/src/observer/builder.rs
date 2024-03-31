use std::{any::TypeId, marker::PhantomData};

use crate::system::IntoObserverSystem;

use super::*;

/// Builder struct for [`Observer`].
pub struct ObserverBuilder<'w, E: EcsEvent = NoEvent> {
    entity: Entity,
    commands: Commands<'w, 'w>,
    descriptor: ObserverDescriptor,
    _marker: PhantomData<E>,
}

impl<'w, E: EcsEvent> ObserverBuilder<'w, E> {
    /// Constructs a new [`ObserverBuilder`].
    pub fn new(mut commands: Commands<'w, 'w>) -> Self {
        let entity = commands.spawn_empty().id();
        Self::new_with_entity(commands, entity)
    }

    pub(crate) fn new_with_entity(commands: Commands<'w, 'w>, entity: Entity) -> Self {
        let mut descriptor = ObserverDescriptor::default();
        // TODO: Better messages
        let event = commands
            .components()
            .get_id(TypeId::of::<E>())
            .unwrap_or_else(|| {
                panic!(
                    "Cannot observe event before it is registered: {}",
                    std::any::type_name::<E>(),
                )
            });

        if event != NO_EVENT {
            descriptor.events.push(event);
        }
        Self {
            entity,
            commands,
            descriptor,
            _marker: PhantomData,
        }
    }

    /// Adds `NewE` to the list of events listened to by this observer.
    /// Observers that listen to multiple types of events can no longer access the typed event data.
    pub fn on_event<NewE: EcsEvent>(&mut self) -> &mut ObserverBuilder<'w, NoEvent> {
        let event = self
            .commands
            .components()
            .get_id(TypeId::of::<NewE>())
            .unwrap_or_else(|| {
                panic!(
                    "Cannot observe event before it is registered: {}",
                    std::any::type_name::<NewE>(),
                )
            });
        self.descriptor.events.push(event);
        // SAFETY: NoEvent type will not allow bad memory access as it has no size
        unsafe { std::mem::transmute(self) }
    }

    /// Add `events` to the list of events listened to by this observer.
    /// Observers that listen to multiple types of events can no longer access the typed event data.
    pub fn on_event_ids(
        &mut self,
        events: impl IntoIterator<Item = ComponentId>,
    ) -> &mut ObserverBuilder<'w, NoEvent> {
        self.descriptor.events.extend(events);
        // SAFETY: () type will not allow bad memory access as it has no size
        unsafe { std::mem::transmute(self) }
    }

    /// Add [`ComponentId`] in `T` to the list of components listened to by this observer.
    pub fn components<B: Bundle>(&mut self) -> &mut Self {
        B::get_component_ids(self.commands.components(), &mut |id| {
            self.descriptor.components.push(id.unwrap_or_else(|| {
                panic!(
                    "Cannot observe event before it is registered: {}",
                    std::any::type_name::<B>(),
                )
            }));
        });
        self
    }

    /// Add `ids` to the list of component sources listened to by this observer.
    pub fn component_ids(&mut self, ids: impl IntoIterator<Item = ComponentId>) -> &mut Self {
        self.descriptor.components.extend(ids);
        self
    }

    /// Adds `source` as the list of entity sources listened to by this observer.
    pub fn source(&mut self, source: Entity) -> &mut Self {
        self.descriptor.sources.push(source);
        self
    }

    /// Spawns the resulting observer into the world.
    pub fn run<B: Bundle, M>(&mut self, callback: impl IntoObserverSystem<E, B, M>) -> Entity {
        B::get_component_ids(self.commands.components(), &mut |id| {
            self.descriptor.components.push(id.unwrap_or_else(|| {
                panic!(
                    "Cannot observe event before it is registered: {}",
                    std::any::type_name::<B>(),
                )
            }));
        });
        let entity = self.entity;
        let descriptor = self.descriptor.clone();
        self.commands.add(move |world: &mut World| {
            let component = ObserverComponent::from(world, descriptor, callback);
            world.entity_mut(entity).insert(component);
        });
        entity
    }

    /// Spawns the resulting observer into the world using a [`ObserverRunner`] callback.
    /// This is not advised unless you want to override the default runner behaviour.
    pub fn runner(&mut self, runner: ObserverRunner) -> Entity {
        let component = ObserverComponent::from_runner(self.descriptor.clone(), runner);
        self.commands.entity(self.entity).insert(component);
        self.entity
    }
}

/// Type used to construct and emit a [`EcsEvent`]
pub struct EventBuilder<'w, E> {
    event: Option<ComponentId>,
    commands: Commands<'w, 'w>,
    targets: Vec<Entity>,
    components: Vec<ComponentId>,
    data: Option<E>,
}

impl<'w, E: EcsEvent> EventBuilder<'w, E> {
    /// Constructs a new builder that will write it's event to `world`'s command queue
    #[must_use]
    pub fn new(data: E, commands: Commands<'w, 'w>) -> Self {
        Self {
            event: None,
            commands,
            targets: Vec::new(),
            components: Vec::new(),
            data: Some(data),
        }
    }

    /// Adds `target` to the list of entities targeted by `self`
    #[must_use]
    pub fn entity(&mut self, target: Entity) -> &mut Self {
        self.targets.push(target);
        self
    }

    /// Sets the event id of the resulting event, used for dynamic events
    /// # Safety
    /// Caller must ensure that the component associated with `id` has the same layout as E
    #[must_use]
    pub unsafe fn event_id(&mut self, id: ComponentId) -> &mut Self {
        self.event = Some(id);
        self
    }

    /// Adds `component_id` to the list of components targeted by `self`
    #[must_use]
    pub fn component(&mut self, component_id: ComponentId) -> &mut Self {
        self.components.push(component_id);
        self
    }

    /// Add the event to the command queue of world
    pub fn emit(&mut self) {
        self.commands.add(EmitEcsEvent::<E> {
            event: self.event,
            data: std::mem::take(&mut self.data).unwrap(),
            entities: std::mem::take(&mut self.targets),
            components: std::mem::take(&mut self.components),
        });
    }
}

impl<'w, 's> Commands<'w, 's> {
    /// Constructs an [`EventBuilder`] for an [`EcsEvent`].
    pub fn event<E: EcsEvent>(&mut self, event: E) -> EventBuilder<E> {
        EventBuilder::new(event, self.reborrow())
    }

    /// Construct an [`ObserverBuilder`]
    pub fn observer_builder<E: EcsEvent>(&mut self) -> ObserverBuilder<E> {
        ObserverBuilder::new(self.reborrow())
    }

    /// Spawn an [`Observer`] and returns it's [`Entity`]
    pub fn observer<E: EcsEvent, B: Bundle, M>(
        &mut self,
        callback: impl IntoObserverSystem<E, B, M>,
    ) -> Entity {
        ObserverBuilder::new(self.reborrow()).run(callback)
    }
}
