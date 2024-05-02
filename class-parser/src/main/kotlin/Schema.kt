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
@SerialName("List")
data class NbtList(@EncodeDefault var inner: NbtElement = NbtAny) : NbtElement() {
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
 * A CompoundTag with an unknown structure. Will be represented as `Map<String, Any>`.
 */
// TODO: sometimes we can still know the specific type of the values.
//  e.g. BlockState properties could be `Map<String, String>` instead of `Map<String, Any>`
@Serializable
@SerialName("AnyCompound")
data object NbtAnyCompound : NbtElement() {
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
     */
    @Transient
    var name: String? = null,
    /**
     * Contains type information about additional keys of which the names aren't statically known.
     */
    // TODO: use this in the Rust world
    var unknownKeys: MutableList<NbtElement> = mutableListOf(),
    /**
     * If any type stores an Entity somewhere in its own structure, the structure is recursive.
     * To represent that in Rust we must introduce a `Box<types::Entity>` alongside the rest of
     * the keys in the compound. This field indicates whether this compound should contain a
     * flattened Entity.
     */
    // TODO: use this in the Rust world
    var nestedEntity: Boolean = false,
) : NbtElement() {
    fun put(name: String, entry: NbtCompoundEntry, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet) {
        entries[name] = entries[name]?.merge(entry, mergeStrategy) ?: entry
    }

    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtAny || other is NbtAnyCompound) return this
        require(other is NbtCompound) { "cannot merge NbtCompound with ${other::class.simpleName}" }
        other.entries.forEach { (k, v) -> put(k, v, mergeStrategy) }
        name = name ?: other.name
        // TODO: really _add_ them all?
        unknownKeys.addAll(other.unknownKeys)
        nestedEntity = nestedEntity || other.nestedEntity
        return this
    }

    fun nameCompounds(compoundTypes: MutableList<CompoundType>) {
        for (entry in entries.values) {
            val elem = entry.value
            if (elem is NbtCompound) {
                if (elem.entries.isEmpty()) {
                    entry.value = NbtAnyCompound
                } else {
                    elem.nameCompounds(compoundTypes)
                    var name = elem.name ?: "Compound${compoundTypes.size}"
                    val new = CompoundType(name, elem.entries, elem.unknownKeys, elem.nestedEntity)
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
                    entry.value = NbtNamedCompound(name)
                }
            } else if (elem is NbtList) {
                // TODO: deduplicate
                val inner = elem.inner
                if (inner is NbtCompound) {
                    if (inner.entries.isEmpty()) {
                        elem.inner = NbtAnyCompound
                    } else {
                        inner.nameCompounds(compoundTypes)
                        var name = inner.name ?: "Compound${compoundTypes.size}"
                        val new = CompoundType(name, inner.entries, inner.unknownKeys, inner.nestedEntity)
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
                        elem.inner = NbtNamedCompound(name)
                    }
                }
            }
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
    val unknownKeys: List<NbtElement> = listOf(),
    var nestedEntity: Boolean = false,
) {
    fun sameNameAs(other: CompoundType): Boolean = other.name.matches(Regex("$name(_\\d+)?"))

    fun sameAs(other: CompoundType): Boolean =
        sameNameAs(other)
                && entries == other.entries
                && unknownKeys == other.unknownKeys
                && nestedEntity == other.nestedEntity
}
