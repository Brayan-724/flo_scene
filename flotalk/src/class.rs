use super::context::*;
use super::continuation::*;
use super::dispatch_table::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::runtime::*;
use super::value::*;
use super::value_messages::*;

use futures::prelude::*;
use smallvec::*;
use once_cell::sync::{Lazy};

use std::any::*;
use std::cell::*;
use std::sync::*;
use std::collections::{HashMap};

//
// TODO: a way to perform a call to another class method without having to use a TalkContinuation::Later would probably be nice
// for performance (and maybe also help with how easy it is to write functions). We currently can't do this because borrowing the
// allocator or the data type also means borrowing the context, and using a RefCell would prevent recursive calls.
//
// There are a few reasons for wanting to do this: the main one is accessing the data of other types, so we can just return
// 'ready' straight away for basic operations like '+' (ie, performance for frequently called, simple, operations).
//
// For other operations, where a lot of computation is generated by a relatively short script, this is probably not necessary
// as the time spent in the larger computation should far outweigh the cost of sending the messages.
//
// flo_scene is really designed around this principle - of high-level things giving low-level things instructions that are simple
// yet take more computation the 'lower' down they are, so we're using the 'deferred' method for now. This is also how Rust async
// blocks work, so there's some precedent (even if not for messages this simple): I think that if this matters for the performance
// of something, then it's more likely that FloTalk is being misused in that situation.
//

/// The ID to assign to the next class that is created
static NEXT_CLASS_ID: Lazy<Mutex<usize>>                                        = Lazy::new(|| Mutex::new(0));

/// A vector containing the boxed class definitions (as an Arc<TClassDefinition>), indexed by class ID
static CLASS_DEFINITIONS: Lazy<Mutex<Vec<Option<Box<dyn Send + Any>>>>>         = Lazy::new(|| Mutex::new(vec![]));

/// A vector containing the callbacks for each class, indexed by class ID (callbacks can be used without knowing the underlying types)
static CLASS_CALLBACKS: Lazy<Mutex<Vec<Option<&'static TalkClassCallbacks>>>>   = Lazy::new(|| Mutex::new(vec![]));

/// A hashmap containing data conversions for fetching the values stored for a particular class (class definition type -> target type -> converter function)
/// The converter function returns a Box<Any> that contains an Option<TargetType> (we use an option so the result can be extracted from the box)
static CLASS_DATA_READERS: Lazy<Mutex<HashMap<TypeId, HashMap<TypeId, Box<dyn Send + Fn(&mut Box<dyn Any>) -> Box<dyn Any>>>>>>
    = Lazy::new(|| Mutex::new(HashMap::new()));


thread_local! {
    static LOCAL_CLASS_CALLBACKS: RefCell<Vec<Option<&'static TalkClassCallbacks>>> = RefCell::new(vec![]);
}

///
/// Callbacks for addressing a TalkClass
///
pub (super) struct TalkClassCallbacks {
    /// Creates the callbacks for this class in a context
    create_in_context: Box<dyn Send + Sync + Fn() -> TalkClassContextCallbacks>,
}

///
/// Callbacks for addressing a TalkClass within a context
///
pub (super) struct TalkClassContextCallbacks {
    /// The dispatch table for this class
    pub (super) dispatch_table: TalkMessageDispatchTable<TalkReference>,

    /// The dispatch table for the class object
    pub (super) class_dispatch_table: TalkMessageDispatchTable<()>,

    /// Add to the reference count for a data handle
    add_reference: Box<dyn Send + Fn(TalkDataHandle, &TalkContext) -> ()>,

    /// Decreases the reference count for a data handle, and frees it if the count reaches 0
    remove_reference: Box<dyn Send + Fn(TalkDataHandle, &TalkContext) -> ()>,

    /// If there's a class data reader for the type ID, return a Box containing an Option<TargetType>, otherwise return None
    read_data: Box<dyn Send + Fn(TypeId, TalkDataHandle) -> Option<Box<dyn Any>>>,

    /// The definition for this class (a boxed Arc<TalkClassDefinition>)
    class_definition: Box<dyn Send + Any>,

    /// The allocator for this class (a boxed Arc<Mutex<TalkClassDefinition::Allocator>>)
    allocator: Box<dyn Send + Any>,
}

///
/// A TalkClass is an identifier for a FloTalk class
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkClass(pub (super) usize);

impl TalkClassCallbacks {
    #[inline]
    pub (super) fn create_in_context(&self) -> TalkClassContextCallbacks {
        (self.create_in_context)()
    }
}

impl TalkClassContextCallbacks {
    #[inline]
    pub (super) fn send_message(&self, reference: TalkReference, message: TalkMessage, context: &TalkContext) -> TalkContinuation<'static> {
        self.dispatch_table.send_message(reference, message, context)
    }

    #[inline]
    pub (super) fn responds_to(&self, message: impl Into<TalkMessageSignatureId>) -> bool {
        self.dispatch_table.responds_to(message)
    }

    #[inline]
    pub (super) fn add_reference(&self, data_handle: TalkDataHandle, context: &TalkContext) {
        (self.add_reference)(data_handle, context)
    }

    #[inline]
    pub (super) fn remove_reference(&self, data_handle: TalkDataHandle, context: &TalkContext) {
        (self.remove_reference)(data_handle, context)
    }

    #[inline]
    pub (super) fn send_class_message(&self, message: TalkMessage, context: &TalkContext) -> TalkContinuation<'static> {
        self.class_dispatch_table.send_message((), message, context)
    }

    #[inline]
    pub (super) fn read_data<TTargetData>(&self, data_handle: TalkDataHandle) -> Option<TTargetData> 
    where
        TTargetData: 'static,
    {
        let data_any = (self.read_data)(TypeId::of::<TTargetData>(), data_handle);

        if let Some(mut data_any) = data_any {
            data_any.downcast_mut::<Option<TTargetData>>().and_then(|data| data.take())
        } else {
            None
        }
    }

    #[inline]
    pub (super) fn allocator<TTargetAllocator>(&mut self) -> Option<Arc<Mutex<TTargetAllocator>>>
    where
        TTargetAllocator: 'static + TalkClassAllocator,
    {
        self.allocator.downcast_mut().cloned()
    }
}

impl TalkClass {
    ///
    /// Creates a new class identifier
    ///
    fn new() -> TalkClass {
        let class_id = {
            let mut next_class_id   = NEXT_CLASS_ID.lock().unwrap();
            let class_id            = *next_class_id;
            *next_class_id          += 1;
            
            class_id
        };

        TalkClass(class_id)
    }
}

///
/// A class definition is a trait implemented by a FloTalk class
///
pub trait TalkClassDefinition : Send + Sync {
    /// The type of the data stored by an object of this class
    type Data: Send;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator: TalkClassAllocator<Data=Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator;

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static>;

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation<'static>;

    ///
    /// Generates default dispatch table for an instance of this class
    ///
    /// Messages are dispatched here ahead of the 'send_instance_message' callback (note in particular `respondsTo:` may need to be overridden)
    ///
    fn default_instance_dispatch_table(&self) -> TalkMessageDispatchTable<TalkReference> { TalkMessageDispatchTable::empty().with_mapped_messages_from(&*TALK_DISPATCH_ANY, |v| TalkValue::Reference(v)) }

    ///
    /// Generates default dispatch table for the class object for this class
    ///
    /// Messages are dispatched here ahead of the 'send_instance_message' callback (note in particular `respondsTo:` may need to be overridden)
    ///
    fn default_class_dispatch_table(&self) -> TalkMessageDispatchTable<()> { TalkMessageDispatchTable::empty() }
}

///
/// A class allocator is used to manage the memory of a class
///
pub trait TalkClassAllocator : Send {
    /// The type of data stored for this class
    type Data: Send;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data;

    ///
    /// Adds to the reference count for a data handle
    ///
    fn add_reference(allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext);

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    fn remove_reference(allocator: &Arc<Mutex<Self>>, handle: TalkDataHandle, context: &TalkContext);
}

impl TalkClass {
    // TODO: we need to share the allocator between several functions, but those functions should all 'exist' in the same thread,
    //       so the allocator should not need to be an Arc<Mutex<...>>: can we use something faster to access? Normally this 
    //       doesn't matter too much but as this ends up in the inner loop of a language interpreter it seems that this could make
    //       a noticeable performance difference.

    ///
    /// Creates the dispatch table for an allocator
    ///
    fn callback_dispatch_table<TClass>(class_id: TalkClass, class_definition: Arc<TClass>, allocator: Arc<Mutex<TClass::Allocator>>) -> TalkMessageDispatchTable<TalkReference> 
    where
        TClass: 'static + TalkClassDefinition,
    {
        class_definition.default_instance_dispatch_table()
            .with_not_supported(move |reference: TalkOwned<TalkReference, &'_ TalkContext>, message_id, message_args, _talk_context| {
                let data_handle     = reference.1;
                let mut allocator   = allocator.lock().unwrap();
                let data            = allocator.retrieve(data_handle);

                class_definition.send_instance_message(message_id, message_args, TalkReference(class_id, data_handle), data)
            })
    }

    ///
    /// Creates the 'add reference' method for an allocator
    ///
    fn callback_add_reference(allocator: Arc<Mutex<impl 'static + TalkClassAllocator>>) -> Box<dyn Send + Fn(TalkDataHandle, &TalkContext) -> ()> {
        Box::new(move |data_handle, context| {
            TalkClassAllocator::add_reference(&allocator, data_handle, context);
        })
    }

    ///
    /// Creates the 'remove reference' method for an allocator
    ///
    fn callback_remove_reference(allocator: Arc<Mutex<impl 'static + TalkClassAllocator>>) -> Box<dyn Send + Fn(TalkDataHandle, &TalkContext) -> ()> {
        Box::new(move |data_handle, context| {
            TalkClassAllocator::remove_reference(&allocator, data_handle, context);
        })
    }

    ///
    /// Creates the 'send class message' function for a class
    ///
    fn callback_class_dispatch_table<TClass>(class_id: TalkClass, definition: Arc<TClass>, allocator: Arc<Mutex<TClass::Allocator>>) -> TalkMessageDispatchTable<()>
    where
        TClass: 'static + TalkClassDefinition,
    {
        definition.default_class_dispatch_table()
            .with_not_supported(move |_: TalkOwned<(), &'_ TalkContext>, message_id, message_args, _talk_context| {
                definition.send_class_message(message_id, message_args, class_id, &allocator)
            })
    }

    ///
    /// Creates the 'read class data' function for a class
    ///
    fn callback_read_data<TClass>(_definition: Arc<TClass>, allocator: Arc<Mutex<TClass::Allocator>>) -> Box<dyn Send + Fn(TypeId, TalkDataHandle) -> Option<Box<dyn Any>>>
    where
        TClass: 'static + TalkClassDefinition,
    {
        let class_type_id = TypeId::of::<TClass>();

        Box::new(move |data_type_id, data_handle| {
            let data_readers = CLASS_DATA_READERS.lock().unwrap();

            if let Some(class_readers) = data_readers.get(&class_type_id) {
                if let Some(target_reader) = class_readers.get(&data_type_id) {
                    // Read the value at the handle
                    let mut parameter: Box<dyn Any> = Box::new((Arc::clone(&allocator), data_handle));
                    let converted_data              = target_reader(&mut parameter);

                    Some(converted_data)
                } else {
                    // No conversions to the target type
                    None
                }
            } else {
                // No conversions for this class
                None
            }
        })
    }

    ///
    /// Creates the 'create in context' function for a class
    ///
    fn callback_create_in_context(class_id: TalkClass, definition: Arc<impl 'static + TalkClassDefinition>) -> Box<dyn Send + Sync + Fn() -> TalkClassContextCallbacks> {
        Box::new(move || {
            let allocator = Arc::new(Mutex::new(definition.create_allocator()));

            TalkClassContextCallbacks {
                dispatch_table:         Self::callback_dispatch_table(class_id, Arc::clone(&definition), Arc::clone(&allocator)),
                class_dispatch_table:   Self::callback_class_dispatch_table(class_id, Arc::clone(&definition), Arc::clone(&allocator)),
                add_reference:          Self::callback_add_reference(Arc::clone(&allocator)),
                remove_reference:       Self::callback_remove_reference(Arc::clone(&allocator)),
                read_data:              Self::callback_read_data(Arc::clone(&definition), Arc::clone(&allocator)),
                class_definition:       Box::new(Arc::clone(&definition)),
                allocator:              Box::new(Arc::clone(&allocator)),
            }
        })
    }

    ///
    /// Creates a TalkClass from a definition
    ///
    pub fn create(definition: impl 'static + TalkClassDefinition) -> TalkClass {
        // Create an identifier for this class
        let definition      = Arc::new(definition);
        let class           = TalkClass::new();
        let TalkClass(idx)  = class;

        // Store the class definition
        let mut class_definitions = CLASS_DEFINITIONS.lock().unwrap();
        while class_definitions.len() <= idx {
            class_definitions.push(None);
        }
        class_definitions[idx] = Some(Box::new(Arc::clone(&definition)));

        // Create the class callbacks
        let class_callbacks = TalkClassCallbacks {
            create_in_context:  Self::callback_create_in_context(class, Arc::clone(&definition)),
        };

        // Store as a static reference (classes live for the lifetime of the program)
        let class_callbacks     = Box::new(class_callbacks);
        let class_callbacks     = Box::leak(class_callbacks);
        let mut all_callbacks   = CLASS_CALLBACKS.lock().unwrap();

        while all_callbacks.len() <= idx {
            all_callbacks.push(None);
        }
        all_callbacks[idx] = Some(class_callbacks);

        // Return the definition we just created
        class
    }

    ///
    /// Looks up the callbacks for this class, 
    ///
    fn make_local_callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx) = *self;

        // Look up the callback in the global set
        let callback = (*CLASS_CALLBACKS.lock().unwrap())[idx].unwrap();

        // Store in the thread-local set so we can retrieve it more quickly in future
        LOCAL_CLASS_CALLBACKS.with(|local_class_callbacks| {
            let mut local_class_callbacks = local_class_callbacks.borrow_mut();

            while local_class_callbacks.len() <= idx {
                local_class_callbacks.push(None);
            }
            local_class_callbacks[idx] = Some(callback);
        });

        // Result is the callback we looked up
        callback
    }

    ///
    /// Retrieve the callbacks for this class
    ///
    #[inline]
    pub (super) fn callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx)  = *self;
        let callback        = LOCAL_CLASS_CALLBACKS.with(|callbacks| {
            let callbacks = callbacks.borrow();

            if idx < callbacks.len() {
                callbacks[idx]
            } else {
                None
            }
        });

        if let Some(callback) = callback {
            callback
        } else {
            self.make_local_callbacks()
        }
    }

    ///
    /// Sends a message to this class
    ///
    #[inline]
    pub fn send_message_in_context<'a>(&self, message: TalkMessage, context: &TalkContext) -> TalkContinuation<'a> {
        if let Some(callbacks) = context.get_callbacks(*self) {
            callbacks.send_class_message(message, context)
        } else {
            let our_class = *self;

            TalkContinuation::Soon(Box::new(move |talk_context| {
                let _ = talk_context.get_callbacks_mut(our_class);
                talk_context.get_callbacks(our_class).unwrap().send_class_message(message, talk_context)
            }))
        }
    }

    ///
    /// Sends a message to this class
    ///
    pub fn send_message<'a>(&self, message: TalkMessage, runtime: &TalkRuntime) -> impl 'a + Future<Output=TalkOwned<TalkValue, TalkOwnedByRuntime>> {
        let class = *self;

        runtime.run(TalkContinuation::<'a>::Soon(Box::new(move |talk_context| {
            let continuation = class.send_message_in_context(message, talk_context);
            continuation
        })))
    }

    ///
    /// Retrieves the definition for this class, or None if the definition is not of the right type
    ///
    pub fn definition<TClass>(&self) -> Option<Arc<TClass>> 
    where
        TClass: 'static + TalkClassDefinition
    {
        let class_definitions = CLASS_DEFINITIONS.lock().unwrap();

        if let Some(Some(any_defn)) = class_definitions.get(self.0) {
            any_defn.downcast_ref::<Arc<TClass>>()
                .map(|defn| Arc::clone(defn))
        } else {
            // Definition not stored/registered
            None
        }
    }

    ///
    /// Retrieves the allocator for this class in a context, or None if the definition is not of the right type
    ///
    pub fn allocator<TAllocator>(&self, context: &mut TalkContext) -> Option<Arc<Mutex<TAllocator>>>
    where
        TAllocator: 'static + TalkClassAllocator
    {
        let callbacks = context.get_callbacks_mut(*self);

        callbacks.allocator.downcast_ref::<Arc<Mutex<TAllocator>>>()
            .map(|defn| Arc::clone(defn))
    }
}

///
/// Adds a function to convert from the internal data type of the specified class to a target data type, for reading the
/// class data for a reference outside of FloTalk (see `TalkReference::read_data()` for where this is used.
///
pub fn talk_add_class_data_reader<TClassDefinition, TTargetData>(read_fn: impl 'static + Send + Fn(&TClassDefinition::Data) -> TTargetData) 
where
    TClassDefinition:   'static + TalkClassDefinition,
    TTargetData:        'static,
{
    // Fetch the types
    let class_definition_type   = TypeId::of::<TClassDefinition>();
    let target_type             = TypeId::of::<TTargetData>();

    // The conversion function takes a reference to a Box<&mut Data> and returns a boxed version of the converted data (for now we only support total conversions)
    let conversion_fn           = move |boxed: &mut Box<dyn Any>| {
        // Unwrap the boxed data
        let (allocator, handle) = boxed.downcast_mut::<(Arc<Mutex<TClassDefinition::Allocator>>, TalkDataHandle)>().unwrap();
        let mut allocator       = allocator.lock().unwrap();
        let data                = allocator.retrieve(*handle);

        // Pass through the conversion function
        let converted           = read_fn(data);

        // Box back up as a Box<Any> (so a generic caller can unwrap it later on)
        let converted: Box<dyn Any> = Box::new(Some(converted));

        converted
    };

    // Add to the class data readers
    CLASS_DATA_READERS.lock().unwrap()
        .entry(class_definition_type)
        .or_insert_with(|| HashMap::new())
        .insert(target_type, Box::new(conversion_fn));
}
