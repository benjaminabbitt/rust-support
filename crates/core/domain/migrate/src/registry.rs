//! The registry: a pair of maps from a detected version to the step that
//! leaves it, `forward[v]` taking an object from version `v` to `v + 1` and
//! `back[v]` from `v` to `v - 1`.
//!
//! # Objects, not trees
//!
//! Every version of a document is a type of its own (`ConfigV1`, `ConfigV2`,
//! ...), and a step is written between two of them with the standard
//! conversion traits: `impl TryFrom<ConfigV1> for ConfigV2` (or `From`, for a
//! step that cannot fail). A **form** (a file format, a database row, a
//! protobuf page) detects the version and loads straight into that version's
//! object; the registry carries the object up to the current type; the form
//! may then store it back. The registry knows no format.
//!
//! ```
//! use migrate::{MigrateError, Registry};
//!
//! struct V1 { name: String }
//! struct V2 { first: String, last: String }
//! impl TryFrom<V1> for V2 {
//!     type Error = MigrateError;
//!     fn try_from(v: V1) -> Result<V2, MigrateError> {
//!         let (first, last) = v.name.split_once(' ').unwrap_or((&v.name, ""));
//!         Ok(V2 { first: first.into(), last: last.into() })
//!     }
//! }
//!
//! let registry = Registry::<V2>::builder(1, 2)
//!     .forward::<V1, V2>(1)
//!     .build()
//!     .unwrap();
//! let migrated = registry.migrate_from(V1 { name: "Ada Lovelace".into() }, 1).unwrap();
//! assert_eq!(migrated.object.last, "Lovelace");
//! ```
//!
//! # The rules, each checked when the registry is built
//!
//! - **One chain.** A step is keyed by the version it starts from and always
//!   ends one away, so no step skips a version, and a version has one forward
//!   step and one back step ([`RegistryError::Duplicate`]).
//! - **Forward is whole.** Every version from the oldest to the one below the
//!   current has a forward step ([`RegistryError::Gap`]), none starts outside
//!   that range ([`RegistryError::OutOfRange`]), and the types line up: each
//!   step makes what the next takes, and the last makes the registry's type
//!   ([`RegistryError::TypeMismatch`], [`RegistryError::NotCurrent`]).
//! - **Back is optional,** and where there is a back step from `v` there is
//!   a forward step from `v - 1` for it to undo
//!   ([`RegistryError::BackWithoutForward`]) between the same two types
//!   ([`RegistryError::BackTypeMismatch`]); it must round-trip, which
//!   [`Registry::round_trip`] checks on a fixture.
//!
//! The stored steps are plain `fn` pointers (the typed step, instantiated for
//! its two types): pure, no captured state, no I/O, clock or environment.
//! The maps are `BTreeMap`s, so iteration is deterministic. Migration is in
//! memory: nothing here reads or writes anything.

use std::any::{type_name, Any, TypeId};
use std::collections::BTreeMap;

use crate::error::{Direction, RegistryError};
use crate::{MigrateError, Version};

/// An object of some version, type-erased while it travels the chain.
pub type Obj = Box<dyn Any>;

/// A stored step: an object of one version in, the next (or previous)
/// version's out.
pub type Step = fn(Obj) -> Result<Obj, MigrateError>;

/// The typed step from `A` to `B`, as the plain function the registry
/// stores: `B::try_from(a)`.
fn convert<A: Any, B: Any + TryFrom<A>>(obj: Obj) -> Result<Obj, MigrateError>
where
    B::Error: Into<MigrateError>,
{
    let a = obj.downcast::<A>().map_err(|_| MigrateError::WrongType {
        expected: type_name::<A>().into(),
    })?;
    B::try_from(*a)
        .map(|b| Box::new(b) as Obj)
        .map_err(Into::into)
}

/// What a migration did.
#[derive(Clone, PartialEq, Debug)]
pub struct Migrated<T> {
    pub object: T,
    /// The version the object was at.
    pub from: Version,
    /// The version it is at now.
    pub to: Version,
    /// The version each step that ran started from, in order.
    pub applied: Vec<Version>,
}

impl<T> Migrated<T> {
    /// Whether any step ran: whether what is stored is out of date.
    pub fn changed(&self) -> bool {
        !self.applied.is_empty()
    }
}

/// What [`Registry::round_trip`] found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundTrip {
    /// The step and its back step gave the object back.
    Held,
    /// There is no back step to try.
    NoBack,
}

/// A step's two types, as the build checks them.
#[derive(Clone, Copy, Debug)]
struct Types {
    from: TypeId,
    to: TypeId,
}

/// A checked registry of steps whose chain ends in the type `C`.
#[derive(Debug)]
pub struct Registry<C> {
    oldest: Version,
    current: Version,
    forward: BTreeMap<Version, Step>,
    back: BTreeMap<Version, Step>,
    names: BTreeMap<Version, &'static str>,
    _current: std::marker::PhantomData<fn() -> C>,
}

/// A registry's steps, not yet checked: [`Builder::build`] checks them.
#[derive(Debug)]
pub struct Builder<C> {
    oldest: Version,
    current: Version,
    forward: Vec<(Version, Step, Types, &'static str)>,
    back: Vec<(Version, Step, Types)>,
    _current: std::marker::PhantomData<fn() -> C>,
}

impl<C: Any> Registry<C> {
    /// A registry of versions `oldest` to `current`, whose objects at the
    /// current version are `C`. Dropping support for old files is raising
    /// `oldest` and deleting their steps.
    pub fn builder(oldest: Version, current: Version) -> Builder<C> {
        Builder {
            oldest,
            current,
            forward: Vec::new(),
            back: Vec::new(),
            _current: std::marker::PhantomData,
        }
    }

    /// The oldest version this registry migrates.
    pub fn oldest(&self) -> Version {
        self.oldest
    }

    /// The version objects migrate to, and are stored at.
    pub fn current(&self) -> Version {
        self.current
    }

    /// Whether a step goes back from `version`.
    pub fn has_back(&self, version: Version) -> bool {
        self.back.contains_key(&version)
    }

    /// The name of the type the registry has for `version`, for a form's
    /// own errors; `None` outside the registry's versions.
    pub fn type_name(&self, version: Version) -> Option<&'static str> {
        if version == self.current {
            Some(type_name::<C>())
        } else {
            self.names.get(&version).copied()
        }
    }

    /// Whether the registry reads `version`: a form asks it after detecting
    /// the version and before loading it.
    ///
    /// # Errors
    /// [`MigrateError::TooNew`] above the current version,
    /// [`MigrateError::TooOld`] below the oldest.
    pub fn readable(&self, version: Version) -> Result<(), MigrateError> {
        if version > self.current {
            return Err(MigrateError::TooNew {
                found: version,
                current: self.current,
            });
        }
        if version < self.oldest {
            return Err(MigrateError::TooOld {
                found: version,
                oldest: self.oldest,
            });
        }
        Ok(())
    }

    /// Carry `object`, which a form loaded as the type of version `from`, up
    /// to `C` through every forward step in order. An object at the current
    /// version comes back as it is.
    ///
    /// # Errors
    /// [`Registry::readable`]'s, [`MigrateError::WrongType`] when `object` is
    /// not the type of `from`, and [`MigrateError::StepFailed`] when a step
    /// fails.
    pub fn migrate(&self, mut object: Obj, from: Version) -> Result<Migrated<C>, MigrateError> {
        self.readable(from)?;
        let mut applied = Vec::new();
        for (&v, step) in self.forward.range(from..) {
            object = step(object).map_err(|cause| MigrateError::StepFailed {
                from: v,
                to: v + 1,
                cause: Box::new(cause),
            })?;
            applied.push(v);
        }
        let object = object
            .downcast::<C>()
            .map_err(|_| MigrateError::WrongType {
                expected: type_name::<C>().into(),
            })?;
        Ok(Migrated {
            object: *object,
            from,
            to: self.current,
            applied,
        })
    }

    /// [`Registry::migrate`] for an object already typed.
    ///
    /// # Errors
    /// As [`Registry::migrate`].
    pub fn migrate_from<A: Any>(
        &self,
        object: A,
        from: Version,
    ) -> Result<Migrated<C>, MigrateError> {
        self.migrate(Box::new(object), from)
    }

    /// Take `object`, of the type of version `from`, back to version `to`,
    /// a `T`, through back steps, the newest first; none runs unless every
    /// one is there.
    ///
    /// # Errors
    /// [`MigrateError::NoBack`] for the highest version with no back step,
    /// [`MigrateError::NotADowngrade`] when `to` is above `from`,
    /// [`MigrateError::WrongType`] when `object` or the result is not the
    /// type of its version, and [`Registry::readable`]'s.
    pub fn downgrade<A: Any, T: Any>(
        &self,
        object: A,
        from: Version,
        to: Version,
    ) -> Result<Migrated<T>, MigrateError> {
        self.readable(from)?;
        self.readable(to)?;
        if to > from {
            return Err(MigrateError::NotADowngrade { from, to });
        }
        // Every step is there before the first runs: a downgrade is whole.
        if let Some(v) = (to + 1..=from).rev().find(|v| !self.back.contains_key(v)) {
            return Err(MigrateError::NoBack { from: v });
        }
        let mut object: Obj = Box::new(object);
        let mut applied = Vec::new();
        if to < from {
            for (&v, step) in self.back.range(to + 1..=from).rev() {
                object = step(object).map_err(|cause| MigrateError::StepFailed {
                    from: v,
                    to: v - 1,
                    cause: Box::new(cause),
                })?;
                applied.push(v);
            }
        }
        let object = object
            .downcast::<T>()
            .map_err(|_| MigrateError::WrongType {
                expected: type_name::<T>().into(),
            })?;
        Ok(Migrated {
            object: *object,
            from,
            to,
            applied,
        })
    }

    /// Whether the forward step from `version` and the back step from
    /// `version + 1` give `object` back: the back rule, on a fixture of the
    /// type of `version`.
    ///
    /// # Errors
    /// [`MigrateError::RoundTrip`] when they do not, and any error of the
    /// two steps; a `version` that is not below the current one is
    /// [`MigrateError::TooNew`].
    pub fn round_trip<A: Any + Clone + PartialEq>(
        &self,
        object: A,
        version: Version,
    ) -> Result<RoundTrip, MigrateError> {
        self.readable(version)?;
        let (Some(up), true) = (self.forward.get(&version), self.has_back(version + 1)) else {
            return if version >= self.current {
                Err(MigrateError::TooNew {
                    found: version,
                    current: self.current,
                })
            } else {
                Ok(RoundTrip::NoBack)
            };
        };
        let stepped = up(Box::new(object.clone())).map_err(|cause| MigrateError::StepFailed {
            from: version,
            to: version + 1,
            cause: Box::new(cause),
        })?;
        let again: A = self.back_one(stepped, version + 1)?;
        if again == object {
            Ok(RoundTrip::Held)
        } else {
            Err(MigrateError::RoundTrip { version })
        }
    }

    fn back_one<T: Any>(&self, object: Obj, from: Version) -> Result<T, MigrateError> {
        let Some(step) = self.back.get(&from) else {
            return Err(MigrateError::NoBack { from });
        };
        let object = step(object).map_err(|cause| MigrateError::StepFailed {
            from,
            to: from - 1,
            cause: Box::new(cause),
        })?;
        let object = object
            .downcast::<T>()
            .map_err(|_| MigrateError::WrongType {
                expected: type_name::<T>().into(),
            })?;
        Ok(*object)
    }
}

impl<C: Any> Builder<C> {
    /// Register the step that takes an `A`, the object of version `from`,
    /// to the `B` of `from + 1`: `B::try_from`. `B::Error` is
    /// [`MigrateError`], or `Infallible` for a `From`.
    #[must_use]
    pub fn forward<A: Any, B: Any + TryFrom<A>>(mut self, from: Version) -> Builder<C>
    where
        B::Error: Into<MigrateError>,
    {
        fn run<A: Any, B: Any + TryFrom<A>>(obj: Obj) -> Result<Obj, MigrateError>
        where
            B::Error: Into<MigrateError>,
        {
            convert::<A, B>(obj)
        }
        self.forward.push((
            from,
            run::<A, B>,
            Types {
                from: TypeId::of::<A>(),
                to: TypeId::of::<B>(),
            },
            type_name::<A>(),
        ));
        self
    }

    /// Register the step that takes a `B`, the object of version `from`, back
    /// to the `A` of `from - 1`: `A::try_from`.
    #[must_use]
    pub fn back<B: Any, A: Any + TryFrom<B>>(mut self, from: Version) -> Builder<C>
    where
        A::Error: Into<MigrateError>,
    {
        fn run<B: Any, A: Any + TryFrom<B>>(obj: Obj) -> Result<Obj, MigrateError>
        where
            A::Error: Into<MigrateError>,
        {
            convert::<B, A>(obj)
        }
        self.back.push((
            from,
            run::<B, A>,
            Types {
                from: TypeId::of::<B>(),
                to: TypeId::of::<A>(),
            },
        ));
        self
    }

    /// Check the steps and make the registry.
    ///
    /// # Errors
    /// The first [`RegistryError`] found, forward steps judged before back.
    pub fn build(self) -> Result<Registry<C>, RegistryError> {
        let Builder {
            oldest,
            current,
            forward,
            back,
            ..
        } = self;
        if oldest > current {
            return Err(RegistryError::Inverted { oldest, current });
        }
        let mut steps = BTreeMap::new();
        for (from, step, types, name) in forward {
            if from < oldest || from >= current {
                return Err(RegistryError::OutOfRange {
                    direction: Direction::Forward,
                    from,
                    oldest,
                    current,
                });
            }
            if steps.insert(from, (step, types, name)).is_some() {
                return Err(RegistryError::Duplicate {
                    direction: Direction::Forward,
                    from,
                });
            }
        }
        if let Some(from) = (oldest..current).find(|v| !steps.contains_key(v)) {
            return Err(RegistryError::Gap { from });
        }
        // The type of each version: what its forward step takes, and `C`
        // at the current one.
        let type_of = |v: Version| match steps.get(&v) {
            Some((_, types, _)) => types.from,
            None => TypeId::of::<C>(),
        };
        for (&from, (_, types, _)) in &steps {
            if types.to != type_of(from + 1) {
                return Err(if from + 1 == current {
                    RegistryError::NotCurrent { from }
                } else {
                    RegistryError::TypeMismatch { from }
                });
            }
        }
        let mut back_steps = BTreeMap::new();
        for (from, step, types) in back {
            // `back[from]` undoes `forward[from - 1]`.
            if from <= oldest || from > current {
                return Err(RegistryError::BackWithoutForward { from });
            }
            if types.from != type_of(from) || types.to != type_of(from - 1) {
                return Err(RegistryError::BackTypeMismatch { from });
            }
            if back_steps.insert(from, step).is_some() {
                return Err(RegistryError::Duplicate {
                    direction: Direction::Back,
                    from,
                });
            }
        }
        Ok(Registry {
            oldest,
            current,
            names: steps.iter().map(|(v, (_, _, name))| (*v, *name)).collect(),
            forward: steps
                .into_iter()
                .map(|(v, (step, _, _))| (v, step))
                .collect(),
            back: back_steps,
            _current: std::marker::PhantomData,
        })
    }
}

#[cfg(test)]
#[path = "registry.test.rs"]
mod tests;
