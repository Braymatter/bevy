use crate::system::{IntoObserverSystem, ObserverSystem};

use super::*;

/// Type for function that is run when an observer is triggered
/// Typically refers to the default runner defined in [`ObserverComponent::from`]
pub type ObserverRunner = fn(DeferredWorld, ObserverTrigger, PtrMut);

pub type BoxedObserverSystem<E = (), B = ()> = Box<dyn ObserverSystem<E, B>>;

pub(crate) struct ObserverComponent {
    pub(crate) descriptor: ObserverDescriptor,
    pub(crate) runner: ObserverRunner,
    pub(crate) system: Option<BoxedObserverSystem>,
    pub(crate) last_event_id: u32,
}

impl Component for ObserverComponent {
    type Storage = SparseStorage;

    fn init_component_info(info: &mut ComponentInfo) {
        info.on_add(|mut world, entity, _| {
            world.commands().add(move |world: &mut World| {
                world.register_observer(entity);
            });
        })
        .on_remove(|mut world, entity, _| {
            world.commands().add(move |world: &mut World| {
                world.unregister_observer(entity);
            });
        });
    }
}

impl ObserverComponent {
    pub(crate) fn from<E: 'static, B: Bundle, M>(
        world: &mut World,
        descriptor: ObserverDescriptor,
        system: impl IntoObserverSystem<E, B, M>,
    ) -> Self {
        let mut system = IntoObserverSystem::into_system(system);
        assert!(
            !system.is_exclusive(),
            "Cannot run exclusive systems in Observers"
        );
        system.initialize(world);
        let system: BoxedObserverSystem<E, B> = Box::new(system);
        Self {
            descriptor,
            runner: |mut world, trigger, ptr| {
                if trigger.source == Entity::PLACEHOLDER {
                    return;
                }
                let world = world.as_unsafe_world_cell();
                let observer_cell =
                    unsafe { world.get_entity(trigger.observer).debug_checked_unwrap() };
                let mut state = unsafe {
                    observer_cell
                        .get_mut::<ObserverComponent>()
                        .debug_checked_unwrap()
                };
                let last_event = unsafe { world.world() }.last_event_id;
                if state.last_event_id == last_event {
                    return;
                }
                state.last_event_id = last_event;

                let observer: Observer<E, B> = Observer::new(unsafe { ptr.deref_mut() }, trigger);
                let mut system: Box<dyn ObserverSystem<E, B>> = unsafe {
                    let system = state.system.take().debug_checked_unwrap();
                    std::mem::transmute(system)
                };

                system.update_archetype_component_access(world);
                unsafe {
                    system.run_unsafe(std::mem::transmute(observer), world);
                    system.queue_deferred(world.into_deferred());
                    state.system = Some(std::mem::transmute(system));
                }
            },
            last_event_id: 0,
            // SAFETY: Same layout
            system: Some(unsafe { std::mem::transmute(system) }),
        }
    }

    pub(crate) fn from_runner(descriptor: ObserverDescriptor, runner: ObserverRunner) -> Self {
        Self {
            descriptor,
            runner,
            last_event_id: 0,
            system: None,
        }
    }
}
