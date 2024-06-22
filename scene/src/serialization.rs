use crate::error::*;
use crate::filter::*;
use crate::input_stream::*;
use crate::scene::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::stream_id::*;

use futures::prelude::*;
use futures::stream;
use futures::stream::{BoxStream};

use once_cell::sync::{Lazy};
use serde::*;

use std::any::*;
use std::collections::{HashMap};
use std::sync::*;

static SERIALIZABLE_MESSAGE_TYPE_NAMES: Lazy<RwLock<HashMap<TypeId, String>>> = Lazy::new(|| RwLock::new(HashMap::new()));

static SEND_SERIALIZED: Lazy<RwLock<HashMap<(TypeId, String), Arc<dyn Send + Sync + Fn(&SceneContext, StreamTarget) -> Result<Box<dyn Send + Any>, ConnectionError>>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the functions for creating serializers of a particular type
static CREATE_ANY_SERIALIZER: Lazy<RwLock<HashMap<TypeId, Arc<dyn Send + Sync + Fn() -> Arc<dyn Send + Sync + Any>>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the functions for transforming a value to and from its serialized representation
static TYPED_SERIALIZERS: Lazy<RwLock<HashMap<(TypeId, TypeId), Arc<dyn Send + Sync + Any>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

/// Stores the filters we've already created so we don't create extr
static FILTERS_FOR_TYPE: Lazy<Mutex<HashMap<(TypeId, TypeId), FilterHandle>>> = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// A message created by serializing another message
///
/// The type ID here can be used if it's necessary to deserialize the message again or determine the original type that was serialized.
///
#[derive(Debug, PartialEq)]
pub struct SerializedMessage<TSerializedType>(pub TSerializedType, pub TypeId);

impl<TSerializedType> SceneMessage for SerializedMessage<TSerializedType> 
where
    TSerializedType: Send + Unpin,
{
}

///
/// Creates a filter that will serialize a message of the specified type
///
/// If a message generates an error when serialized, this will ignore it.
///
/// The filter generated here will create `SerializedMessage` messages, mapped to a final output type via the map_stream message. This example
/// leaves the message as a 'SerializedMessage':
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene::programs::*;
/// #
/// # use serde::*;
/// # use serde_json;
/// #
/// # #[derive(Serialize)]
/// # enum TestMessage { Test }
/// # impl SceneMessage for TestMessage { }
/// #
/// let serialize_filter = serializer_filter::<TestMessage, _, _>(|| serde_json::value::Serializer, |stream| stream);
/// ```
///
pub fn create_serializer_filter<TMessageType, TSerializer, TTargetStream>(serializer: impl 'static + Send + Sync + Fn() -> TSerializer, map_stream: impl 'static + Send + Sync + Fn(BoxStream<'static, SerializedMessage<TSerializer::Ok>>) -> TTargetStream) -> FilterHandle
where
    TMessageType:           'static + SceneMessage + Serialize,
    TSerializer:            'static + Send + Serializer,
    TSerializer::Ok:        'static + Send + Unpin,
    TTargetStream:          'static + Send + Stream,
    TTargetStream::Item:    'static + SceneMessage,
{
    let serializer  = Arc::new(serializer);
    let type_id     = TypeId::of::<TMessageType>();

    // The filter creates a serializer per message, then passes the stream through the `map_stream` function to generate the final message type
    // map_stream is here because otherwise it's quite hard to accept serialized messages along with other types as we can't combine filters
    FilterHandle::for_filter(move |message_stream: InputStream<TMessageType>| {
        let serializer = serializer.clone();

        let serialized_stream = message_stream
            .map(move |message| {
                let serializer  = (serializer)();
                let serialized  = message.serialize(serializer).ok()
                    .map(|serialized| SerializedMessage(serialized, type_id));

                stream::iter(serialized)
            })
            .flatten()
            .boxed();

        map_stream(serialized_stream)
    })
}

///
/// Creates a filter that can be used to deserialize incoming messages of a particular type
///
/// The mapping stream can be used to further change the message type if neeeded.
///
/// If a message has the wrong type ID attached to it, or generates an error when deserializing, this will ignore it.
///
/// ```
/// # use flo_scene::*;
/// # use flo_scene::programs::*;
/// #
/// # use serde::*;
/// # use serde_json;
/// #
/// # #[derive(Serialize, Deserialize)]
/// # enum TestMessage { Test }
/// # impl SceneMessage for TestMessage { }
/// #
/// let deserialize_filter = deserializer_filter::<TestMessage, serde_json::Value, _>(|stream| stream);
/// ```
///
pub fn deserializer_filter<TMessageType, TSerializedValue, TTargetStream>(map_stream: impl 'static + Send + Sync + Fn(BoxStream<'static, TMessageType>) -> TTargetStream) -> FilterHandle
where
    TMessageType:           'static + SceneMessage + for<'a> Deserialize<'a>,
    TSerializedValue:       'static + Send + Unpin + for<'a> Deserializer<'a>,
    TTargetStream:          'static + Send + Stream,
    TTargetStream::Item:    'static + SceneMessage,
{
    let type_id     = TypeId::of::<TMessageType>();

    FilterHandle::for_filter(move |message_stream: InputStream<SerializedMessage<TSerializedValue>>| {
        let deserialized_stream = message_stream
            .map(move |SerializedMessage(message_value, message_type)| {
                if message_type != type_id {
                    stream::iter(None)
                } else {
                    stream::iter(TMessageType::deserialize(message_value).ok())
                }
            })
            .flatten()
            .boxed();

        map_stream(deserialized_stream)
    })
}

///
/// Adds a constructor for a serializer to the types that flo_scene knows about
///
/// flo_scene can't use serializers that need setting up with state for the default way that messages are serialized,
/// but this allows it to automatically fill in all of the serializers for a single type.
///
/// This can be called multiple times for a serializer if necessary: the existing serializer will be replaced with
/// whatever is passed in.
///
pub fn install_serializer<TSerializer>(create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) 
where
    TSerializer:        'static + Send + Serializer,
    TSerializer::Ok:    'static + Send + Unpin,
    TSerializer::Ok:    for<'a> Deserializer<'a>,
{
    let mut create_any_serializer = (*CREATE_ANY_SERIALIZER).write().unwrap();

    let create_serializer_fn: Box<dyn Send + Sync + Fn() -> TSerializer>    = Box::new(create_serializer);
    let create_serializer_fn: Arc<dyn Send + Sync + Any>                    = Arc::new(create_serializer_fn);

    // Add a function that creates a boxed Any that creates this serializer type
    create_any_serializer.insert(TypeId::of::<TSerializer>(), 
        Arc::new(move || Arc::clone(&create_serializer_fn)));
}

// TODO: would be nice to not have to install the type for each type of serializable type we want to add but I'm currently not sure how to do this.
// It's probably possible if we hard-code JSON as our serialization target

///
/// Creates the data structures needed to serialize a particular type
///
/// The serializer must have previously been installed with `install_serializer` so that `flo_scene` knows how to
/// create an instance of it. The type name must be unique and is associated with the serialized type: it's used
/// when deciding how to deserialize a value.
///
/// It's necessary to install a version of the serializable type for each serializer that's in use. The type name must
/// identify a single message type and cannot be used for a different `TMessageType` 
///
pub fn install_serializable_type<TMessageType, TSerializer>(type_name: impl Into<String>) -> Result<(), &'static str>
where
    TMessageType:                   'static + SceneMessage,
    TMessageType:                   for<'a> Deserialize<'a>,
    TMessageType:                   Serialize,
    TSerializer:                    'static + Send + Serializer,
    TSerializer::Ok:                'static + Send + Unpin,
    for<'a> &'a TSerializer::Ok:    Deserializer<'a>,
{
    // Store the name for this type (which must match the old name)
    let type_name = type_name.into();
    {
        let mut type_names = (*SERIALIZABLE_MESSAGE_TYPE_NAMES).write().unwrap();

        if let Some(existing_type_name) = type_names.get(&TypeId::of::<TMessageType>()) {
            if existing_type_name != &type_name {
                return Err("Serialization type name has been used by another type");
            }
        } else {
            type_names.insert(TypeId::of::<TMessageType>(), type_name);
        }
    }

    // Fetch the serializer constructor function (this is what's set up by install_serializer)
    let new_serializer = (*CREATE_ANY_SERIALIZER).read().unwrap()
        .get(&TypeId::of::<TSerializer>())
        .cloned();
    let new_serializer = if let Some(new_serializer) = new_serializer { new_serializer } else { return Err("Serializer has not been installed by install_serializer()"); };
    let new_serializer = new_serializer().downcast::<Box<dyn Send + Sync + Fn() -> TSerializer>>();
    let new_serializer = if let Ok(new_serializer) = new_serializer { new_serializer } else { return Err("Serializer was not installed correctly"); };

    // Create closures for creating a mapping between the input and the output type
    let typed_serializer = move |input: TMessageType| -> Result<TSerializer::Ok, TMessageType> {
        if let Ok(val) = input.serialize(new_serializer()) {
            Ok(val)
        } else {
            Err(input)
        }
    };

    // Create another closure for deserializing
    let typed_deserializer = move |input: TSerializer::Ok| -> Result<TMessageType, TSerializer::Ok> {
        use std::mem;

        let val = TMessageType::deserialize(&input);

        match val {
            Ok(val) => Ok(val),
            Err(_)  => {
                mem::drop(val);
                Err(input)
            },
        }
    };

    // Convert to boxed functions
    let typed_serializer: Box<dyn Send + Sync + Fn(TMessageType) -> Result<TSerializer::Ok, TMessageType>>        = Box::new(typed_serializer);
    let typed_deserializer: Box<dyn Send + Sync + Fn(TSerializer::Ok) -> Result<TMessageType, TSerializer::Ok>>   = Box::new(typed_deserializer);

    // Set as an 'any' type for storage
    let typed_serializer: Arc<dyn Send + Sync + Any>    = Arc::new(typed_serializer);
    let typed_deserializer: Arc<dyn Send + Sync + Any>  = Arc::new(typed_deserializer);

    // Store the serializer and deserializer in the typed serializers list
    let mut typed_serializers = (*TYPED_SERIALIZERS).write().unwrap();

    typed_serializers.insert((TypeId::of::<TMessageType>(), TypeId::of::<TSerializer::Ok>()), typed_serializer);
    typed_serializers.insert((TypeId::of::<TSerializer::Ok>(), TypeId::of::<TMessageType>()), typed_deserializer);

    Ok(())
}

///
/// If installed, returns a filter to convert from a source type to a target type
///
/// This will create either a serializer or a deserializer depending on the direction that the conversion goes in
///
pub fn serializer_filter<TSourceType, TTargetType>() -> Result<FilterHandle, &'static str> 
where
    TSourceType: 'static + SceneMessage,
    TTargetType: 'static + SceneMessage,
{
    let mut filters_for_type = (*FILTERS_FOR_TYPE).lock().unwrap();

    // The message type is the key for retrieving this filter later on
    let message_type = (TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>());

    if let Some(filter) = filters_for_type.get(&message_type) {
        // Use the existing filter
        Ok(*filter)
    } else {
        // Create a new filter
        let typed_serializer = (*TYPED_SERIALIZERS).read().unwrap().get(&(TypeId::of::<TSourceType>(), TypeId::of::<TTargetType>())).cloned();
        let typed_serializer = if let Some(typed_serializer) = typed_serializer { Ok(typed_serializer) } else { Err("The requested serializers are not installed") }?;
        let typed_serializer = if let Ok(typed_serializer) = typed_serializer.downcast::<Box<dyn Send + Sync + Fn(TSourceType) -> Result<TTargetType, TSourceType>>>() { 
            Ok(typed_serializer)
        } else {
            Err("Could not properly resolve the type of the requested serializer")
        }?;

        // Create a filter that uses the stored serializer
        let filter_handle = FilterHandle::for_filter(move |input_messages| {
            let typed_serializer = Arc::clone(&typed_serializer);

            input_messages.flat_map(move |msg| stream::iter((*typed_serializer)(msg).ok()))
        });

        // Store for future use
        filters_for_type.insert(message_type, filter_handle);

        // Result is the new filter
        Ok(filter_handle)
    }
}


///
/// Install serializers and deserializers so that messages of a particular type can be filtered to and from `SerializedMessage<TSerializer::Ok>`
///
/// The type name is associated with the filters created by this function and can be used to create a sink that sends the raw serialized messages. This name
/// must be unique: use something like `crate_name::type_name` for this value to ensure that there are no conflicts.
///
pub fn install_serializers<TMessageType, TSerializer>(scene: &Scene, type_name: &str, create_serializer: impl 'static + Send + Sync + Fn() -> TSerializer) -> Result<(), ConnectionError>
where
    TMessageType:       'static + SceneMessage,
    TMessageType:       for<'a> Deserialize<'a>,
    TMessageType:       Serialize,
    TSerializer:        'static + Send + Serializer,
    TSerializer::Ok:    'static + Send + Unpin,
    TSerializer::Ok:    for<'a> Deserializer<'a>,
{
    use std::mem;

    // Stores the currently known filters
    static FILTERS_FOR_TYPE: Lazy<RwLock<HashMap<(TypeId, TypeId), (FilterHandle, FilterHandle)>>> = 
        Lazy::new(|| RwLock::new(HashMap::new()));

    // Fetch the existing filters if there are any for this type
    let message_type        = TypeId::of::<TMessageType>();
    let serializer_type     = TypeId::of::<TSerializer>();
    let filters_for_type    = FILTERS_FOR_TYPE.read().unwrap();

    let (serialize_filter, deserialize_filter) = if let Some(filters) = filters_for_type.get(&(message_type, serializer_type)) {
        // Use the known filters
        *filters
    } else {
        // Try again with the write lock (to avoid a race condition)
        mem::drop(filters_for_type);
        let mut filters_for_type = FILTERS_FOR_TYPE.write().unwrap();

        if let Some(filters) = filters_for_type.get(&(message_type, serializer_type)) {
            // Rare race condition occurred and the filters were being created on another thread
            *filters
        } else {
            // Create some new filters for this message type
            let serialize_filter    = create_serializer_filter::<TMessageType, _, _>(move || create_serializer(), move |stream| stream);
            let deserialize_filter  = deserializer_filter::<TMessageType, TSerializer::Ok, _>(|stream| stream);

            // Cache them
            filters_for_type.insert((message_type, serializer_type), (serialize_filter, deserialize_filter));

            // Use them as the filters to connect
            (serialize_filter, deserialize_filter)
        }
    };

    // Create a function to generate a sink to deserialize messages of this type
    {
        let mut send_serialized = SEND_SERIALIZED.write().unwrap();
        let filter_sink         = Arc::new(|scene_context: &SceneContext, target: StreamTarget| -> Result<Box<dyn Send + Any>, ConnectionError> {
            // Create a sink to send the message type to
            let sink = scene_context.send::<TMessageType>(target)?;

            // Map it to the deserializer
            // TODO: report deserialization errors
            let sink = sink.with_flat_map(|msg: TSerializer::Ok| {
                stream::iter(TMessageType::deserialize(msg).ok().map(|msg| Ok(msg)))
            }).sink_map_err(|err| {
                // TODO: the other error types require getting back the serialized values
                SceneSendError::TargetProgramEndedBeforeReady
            });

            // Convert to an 'any' sink
            let sink: Box<dyn Unpin + Send + Sink<TSerializer::Ok, Error=SceneSendError<TSerializer::Ok>>> = Box::new(sink);
            let sink: Box<dyn Send + Any> = Box::new(sink);

            Ok(sink)
        });

        send_serialized.insert((TypeId::of::<TSerializer::Ok>(), type_name.to_string()), filter_sink);
    }

    // Add source filters to serialize and deserialize to the scene
    scene.connect_programs(StreamSource::Filtered(serialize_filter), (), StreamId::with_message_type::<TMessageType>())?;
    scene.connect_programs(StreamSource::Filtered(deserialize_filter), (), StreamId::with_message_type::<SerializedMessage<TSerializer::Ok>>())?;

    Ok(())
}

impl SceneContext {
    ///
    /// Creates an output sink that receives messages serialized using a serde serializer, and sends them using the native type.
    ///
    /// The serializer needs to be installed using `install_serializers` with a matching `type_name`.
    ///
    pub fn send_serialized<TSerializedType>(&self, type_name: impl Into<String>, target: impl Into<StreamTarget>) -> Result<impl 'static + Unpin + Send + Sink<TSerializedType, Error=SceneSendError<TSerializedType>>, ConnectionError>
    where
        TSerializedType:    'static + Send + Unpin,
    {
        // Try to fetch the function that creates the sink for this type
        let send_serialized = SEND_SERIALIZED.read().unwrap();

        if let Some(create_sink) = send_serialized.get(&(TypeId::of::<TSerializedType>(), type_name.into())) {
            // Create a sink for the type name
            let any_sink    = (create_sink)(self, target.into())?;
            let boxed_sink  = any_sink.downcast::<Box<dyn Unpin + Send + Sink<TSerializedType, Error=SceneSendError<TSerializedType>>>>().unwrap();

            Ok(*boxed_sink)
        } else {
            // This type is not available
            Err(ConnectionError::TargetNotAvailable)
        }
    }
}

impl StreamId {
    ///
    /// If this stream can be serialized, then this is the serialization type name that can be used to specify it
    ///
    pub fn serialization_type_name(&self) -> Option<String> {
        None
    }

    ///
    /// Changes a serialization name into a stream ID
    ///
    pub fn with_serialization_type(type_name: impl Into<String>) -> Option<Self> {
        None
    }
}
