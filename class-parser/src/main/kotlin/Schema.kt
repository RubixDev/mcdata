@file:OptIn(ExperimentalSerializationApi::class)

package de.rubixdev

import kotlinx.serialization.*

/////////////////////////
///////// INPUT /////////
/////////////////////////

@Serializable
data class InputEntityInfo(
    val entities: List<InputEntity>,
    val classes: Map<String, String>,
)

@Serializable
data class InputEntity(
    val id: String,
    @SerialName("class")
    val className: String,
    val experimental: Boolean = false,
)

//////////////////////////
///////// OUTPUT /////////
//////////////////////////

@Serializable
data class EntityInfo(
    val entities: List<Entity>,
    val types: List<EntityType>,
    val compoundTypes: List<CompoundType>,
)

@Serializable
data class Entity(
    val id: String,
    val type: String,
    val experimental: Boolean = false,
)

@Serializable
data class EntityType(
    val name: String,
    val parent: String? = null,
    val nbt: NbtCompound,
)

@Serializable
sealed class NbtElement {
    open fun merge(other: NbtElement, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet): NbtElement {
        require(other == NbtAny || this::class == other::class) {
            "cannot merge ${this::class.simpleName} with ${other::class.simpleName}"
        }
        return this
    }

    /**
     * Combine this type and [other] into one type which can represent all values
     * from both types.
     */
    fun encompass(other: NbtElement?): NbtElement {
        if (other == null) return this
        return when (this) {
            NbtAny -> this
            NbtByte -> if (other == NbtByte) this else NbtAny
            NbtShort -> if (other == NbtShort) this else NbtAny
            NbtInt -> if (other == NbtInt) this else NbtAny
            NbtLong -> if (other == NbtLong) this else NbtAny
            NbtFloat -> if (other == NbtFloat) this else NbtAny
            NbtDouble -> if (other == NbtDouble) this else NbtAny
            NbtString -> if (other == NbtString) this else NbtAny
            NbtByteArray -> if (other == NbtByteArray) this else NbtAny
            NbtIntArray -> if (other == NbtIntArray) this else NbtAny
            NbtLongArray -> if (other == NbtLongArray) this else NbtAny
            NbtUuid -> if (other == NbtUuid) this else NbtAny
            NbtBoolean -> if (other == NbtBoolean) this else NbtAny
            is NbtNamedCompound -> throw IllegalStateException("named compound before finished running")
            is NbtList -> if (other is NbtList) NbtList(inner.encompass(other.inner)) else NbtAny
            // TODO: the next two could also be NbtAnyCompound, but I don't think that ever happens anyway
            is NbtBoxed -> if (this == other) this else NbtAny
            is NbtNestedEntity -> if (this == NbtNestedEntity) this else NbtAny
            is NbtEither -> when (other) {
                left, right -> this
                is NbtEither -> NbtEither(left.encompass(other.left), right.encompass(other.right))
                else -> NbtAny
            }

            is NbtAnyCompound -> when (other) {
                is NbtAnyCompound -> NbtAnyCompound(valueType.encompass(other.valueType))
                is NbtCompound -> NbtAnyCompound(other.entries.values.fold(valueType) { acc, it -> acc.encompass(it.value) }
                    .encompass(other.unknownKeys))

                else -> NbtAny
            }

            is NbtCompound -> when (other) {
                is NbtAnyCompound -> NbtAnyCompound(entries.values.fold(other.valueType) { acc, it -> acc.encompass(it.value) }
                    .encompass(unknownKeys))

                is NbtCompound -> if (this == other) this else {
                    NbtAnyCompound(
                        entries.values.plus(other.entries.values).run {
                            if (isEmpty()) {
                                NbtAny
                            } else {
                                fold(first().value) { acc, it -> acc.encompass(it.value) }
                            }
                        }
                    )
                }

                else -> NbtAny
            }
        }
    }

    fun clone(): NbtElement =
        when (this) {
            NbtAny, NbtByte, NbtShort, NbtInt, NbtLong, NbtFloat, NbtDouble, NbtString, NbtByteArray, NbtIntArray,
            NbtLongArray, NbtUuid, NbtBoolean, NbtNestedEntity -> this

            is NbtEither -> NbtEither(left.clone(), right.clone())
            is NbtBoxed -> NbtBoxed(name)
            is NbtList -> NbtList(inner.clone())
            is NbtAnyCompound -> NbtAnyCompound(valueType.clone())
            is NbtNamedCompound -> NbtNamedCompound(name)
            is NbtCompound -> NbtCompound(
                entries.map { (k, e) -> k to NbtCompoundEntry(e.value.clone(), e.optional) }.toMap(mutableMapOf()),
                name,
                unknownKeys?.clone(),
                flattened.map { it.clone() }.toMutableList(),
            )
        }
}

@Serializable
@SerialName("Any")
data object NbtAny : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement = other
}

// @formatter:off
@Serializable @SerialName("Byte")      data object NbtByte      : NbtElement()
@Serializable @SerialName("Short")     data object NbtShort     : NbtElement()
@Serializable @SerialName("Int")       data object NbtInt       : NbtElement()
@Serializable @SerialName("Long")      data object NbtLong      : NbtElement()
@Serializable @SerialName("Float")     data object NbtFloat     : NbtElement()
@Serializable @SerialName("Double")    data object NbtDouble    : NbtElement()
@Serializable @SerialName("String")    data object NbtString    : NbtElement()
@Serializable @SerialName("ByteArray") data object NbtByteArray : NbtElement()
@Serializable @SerialName("IntArray")  data object NbtIntArray  : NbtElement()
@Serializable @SerialName("LongArray") data object NbtLongArray : NbtElement()
@Serializable @SerialName("Uuid")      data object NbtUuid      : NbtElement()
@Serializable @SerialName("Boolean")   data object NbtBoolean   : NbtElement()
// @formatter:on

@Serializable
@SerialName("Either")
data class NbtEither(
    var left: NbtElement,
    var right: NbtElement,
) : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other == NbtAny || other == left || other == right) return this
        require(other is NbtEither) { "cannot merge NbtEither with ${other::class.simpleName}" }
        return NbtEither(left.merge(other.left, mergeStrategy), right.merge(other.right, mergeStrategy))
    }
}

/**
 * A `Box<T>` where `T` is another compound with the name [name].
 * This is needed for recursive structures.
 */
@Serializable
@SerialName("Boxed")
// TODO: I think technically this should also store an `optional` bool for `overrideOptional`
data class NbtBoxed(val name: String) : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtBoxed && other.name != name) {
            throw IllegalArgumentException("cannot merge two NbtBoxed with different names")
        }
        return super.merge(other, mergeStrategy)
    }
}

/**
 * A `Box<Entity>` that can fully represent _any_ other entity including its id.
 */
@Serializable
@SerialName("NestedEntity")
data object NbtNestedEntity : NbtElement()

@Serializable
@SerialName("List")
data class NbtList(
    @EncodeDefault
    var inner: NbtElement = NbtAny,
) : NbtElement() {
    fun add(value: NbtElement, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet) {
        inner = inner.merge(value, mergeStrategy)
    }

    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other == NbtAny) return this
        require(other is NbtList) { "cannot merge NbtList with ${other::class.simpleName}" }
        add(other.inner)
        return this
    }
}

/**
 * A CompoundTag with an unknown structure. Will be represented as `Map<String, T>` where `T`
 * is the type stored in [valueType].
 */
@Serializable
@SerialName("AnyCompound")
data class NbtAnyCompound(
    @EncodeDefault
    val valueType: NbtElement = NbtAny,
) : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtCompound) return other
        return super.merge(other, mergeStrategy)
    }
}

@Serializable
data class NbtCompound(
    @EncodeDefault
    val entries: MutableMap<String, NbtCompoundEntry> = mutableMapOf(),
    /**
     * Can be used to give this compound a more descriptive name.
     * The type is a [MethodCall], the actual name is derived from that by calling [MethodCall.toTypeName]
     * before converting to an [NbtNamedCompound].
     */
    @Transient
    var name: MethodCall? = null,
    /**
     * If this compound may contain extra keys which aren't statically known, we record which type
     * they all have in common (similar to [NbtAnyCompound.valueType]). These keys are then stored
     * in a flattened `HashMap<String, T>`.
     */
    var unknownKeys: NbtElement? = null,
    /**
     * A compound can also contain entries from other compounds as if they were in this one.
     * This is mainly needed for recursive structures using [NbtBoxed].
     * Elements in this list should only be [NbtCompound] or [NbtBoxed].
     */
    val flattened: MutableList<NbtElement> = mutableListOf(),
) : NbtElement() {
    fun put(name: String, entry: NbtCompoundEntry, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet) {
        entries[name] = entries[name]?.merge(entry, mergeStrategy) ?: entry
    }

    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtAny || other is NbtAnyCompound) return this
        require(other is NbtCompound) { "cannot merge NbtCompound with ${other::class.simpleName}" }
        other.entries.forEach { (k, v) -> put(k, v, mergeStrategy) }
        name = name ?: other.name
        unknownKeys = unknownKeys?.encompass(other.unknownKeys) ?: other.unknownKeys
        flattened += other.flattened
        return this
    }

    /**
     * Flattens all contained [flattened] compounds unless they are needed for recursive structures.
     */
    fun flatten(boxedTypes: Set<MethodCall>) {
        forEachChildCompound { compound, _ -> compound.flatten(boxedTypes) }
        val oldFlattened = flattened.toList()
        require(oldFlattened !== flattened)
        flattened.clear()
        for (elem in oldFlattened) {
            when (elem) {
                is NbtCompound -> if (boxedTypes.contains(elem.name)) {
                    flattened.add(elem)
                } else {
                    merge(elem)
                }

                is NbtBoxed -> flattened.add(elem)
                else -> throw IllegalStateException("flattened element which is neither NbtCompound nor NbtBoxed")
            }
        }
    }

    fun nameCompounds(compoundTypes: MutableList<CompoundType>) {
        forEachChildCompound { compound, replaceSelf -> compound.nameSelf(compoundTypes, replaceSelf) }
    }

    private fun forEachChildCompound(
        consumer: (compound: NbtCompound, replaceSelf: (NbtElement) -> Unit) -> Unit,
    ) {
        for (entry in entries.values) {
            val elem = entry.value
            if (elem is NbtCompound) {
                consumer(elem) { entry.value = it }
            } else if (elem is NbtList) {
                val inner = elem.inner
                if (inner is NbtCompound) {
                    consumer(inner) { elem.inner = it }
                }
            } else if (elem is NbtEither) {
                val left = elem.left
                val right = elem.right
                if (left is NbtCompound) {
                    consumer(left) { elem.left = it }
                }
                if (right is NbtCompound) {
                    consumer(right) { elem.right = it }
                }
            }
        }
        for ((idx, elem) in flattened.withIndex()) {
            if (elem is NbtCompound) {
                consumer(elem) { flattened[idx] = it }
            }
        }
    }

    private fun nameSelf(compoundTypes: MutableList<CompoundType>, replaceSelf: (NbtElement) -> Unit) {
        nameCompounds(compoundTypes)
        if (entries.isEmpty() && unknownKeys == null && flattened.size == 1) {
            replaceSelf(flattened[0])
        } else if (entries.isEmpty() && flattened.isEmpty()) {
            replaceSelf(NbtAnyCompound(unknownKeys ?: NbtAny))
        } else {
            var name = name?.toTypeName() ?: "Compound${compoundTypes.size}"
            val new = CompoundType(name, entries, unknownKeys, flattened)
            val sameName = compoundTypes.filter { new.sameNameAs(it) }
            if (sameName.isEmpty()) {
                compoundTypes.add(new)
            } else {
                val existing = sameName.find { new.sameAs(it) }
                if (existing != null) {
                    name = existing.name
                } else {
                    name += "_${sameName.size}"
                    new.name = name
                    compoundTypes.add(new)
                }
            }
            replaceSelf(NbtNamedCompound(name))
        }
    }
}

@Serializable
data class NbtCompoundEntry(
    var value: NbtElement,
    var optional: Boolean = false,
) {
    fun merge(other: NbtCompoundEntry, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet) = NbtCompoundEntry(
        value = value.merge(other.value),
        optional = when (mergeStrategy) {
            // this XOR is to prevent code like `if (x) put("a", y) else put("a", z)` where "a" is added twice in the
            // same method and both times marked as optional, but it isn't actually optional
            MergeStrategy.SameDataSet -> optional xor other.optional
            MergeStrategy.DifferentDataSet -> optional || other.optional
        },
    )
}

enum class MergeStrategy {
    SameDataSet,
    DifferentDataSet,
}

@Serializable
@SerialName("Compound")
data class NbtNamedCompound(val name: String) : NbtElement()

/**
 * A replacement for [NbtCompound] which also stores a name for how the Rust
 * struct representing this compound should be named. The [NbtCompound]s are
 * replaced by [NbtNamedCompound]s pointing to the corresponding [CompoundType]
 * via its name.
 */
@Serializable
data class CompoundType(
    var name: String,
    val entries: Map<String, NbtCompoundEntry>,
    val unknownKeys: NbtElement? = null,
    val flattened: List<NbtElement> = listOf(),
) {
    fun sameNameAs(other: CompoundType): Boolean = other.name.matches(Regex("$name(_\\d+)?"))

    fun sameAs(other: CompoundType): Boolean =
        sameNameAs(other)
                && entries == other.entries
                && unknownKeys == other.unknownKeys
                && flattened == other.flattened
}
