package de.rubixdev

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

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

@Serializable
@SerialName("List")
data class NbtList(val inner: NbtElement) : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other == NbtAny) return this
        require(other is NbtList) { "cannot merge NbtList with ${other::class.simpleName}" }
        return NbtList(inner.merge(other.inner, mergeStrategy))
    }
}

// for compounds with unknown fields
@Serializable
@SerialName("AnyCompound")
data object NbtAnyCompound : NbtElement() {
    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtCompound) return other
        return super.merge(other, mergeStrategy)
    }
}

@Serializable
data class NbtCompound(val entries: MutableMap<String, NbtCompoundEntry>) : NbtElement() {
    fun add(name: String, entry: NbtCompoundEntry, mergeStrategy: MergeStrategy = MergeStrategy.SameDataSet) {
        entries[name] = entries[name]?.merge(entry, mergeStrategy) ?: entry
    }

    override fun merge(other: NbtElement, mergeStrategy: MergeStrategy): NbtElement {
        if (other is NbtAny || other is NbtAnyCompound) return this
        require(other is NbtCompound) { "cannot merge NbtCompound with ${other::class.simpleName}" }
        other.entries.forEach { (k, v) -> add(k, v, mergeStrategy) }
        return this
    }

    fun nameCompounds(compoundTypes: MutableList<CompoundType>) {
        for (entry in entries.values) {
            val elem = entry.value
            if (elem is NbtCompound) {
                val name = "Compound${compoundTypes.size}"
                compoundTypes.add(CompoundType(name, elem.entries))
                elem.nameCompounds(compoundTypes)
                entry.value = NbtNamedCompound(name)
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
        optional = when(mergeStrategy) {
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
    val name: String,
    val entries: Map<String, NbtCompoundEntry>,
) : NbtElement()
