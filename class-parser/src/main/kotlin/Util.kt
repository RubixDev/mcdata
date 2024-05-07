package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.Method
import org.apache.bcel.classfile.StackMapType
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.LocalVariables
import org.apache.bcel.verifier.structurals.OperandStack
import org.apache.bcel.verifier.structurals.UninitializedObjectType

fun printWarning(msg: String) = println("\u001b[1;33m>>>> WARNING: $msg\u001b[0m")

object UninitializedLocal : Type(Const.T_UNKNOWN, "<uninitialized local>")

fun classToTypeName(className: String): String = className.split('.', '/').last().split('$').last()

data class MethodPointer(
    val className: String,
    val name: String,
    val signature: String,
) {
    override fun toString(): String = "$className.$name$signature"
}

class StringTypeWithValue(val value: String) : ObjectType("java.lang.String") {
    override fun toString(): String = "java.lang.String = \"$value\""

    override fun equals(other: Any?): Boolean {
        return if (other is StringTypeWithValue) value == other.value else super.equals(other)
    }

    override fun hashCode(): Int = System.identityHashCode(this)
}

class IntTypeWithValue(val value: Int) : Type(Const.T_INT, "I") {
    override fun toString(): String = "int = $value"

    override fun equals(other: Any?): Boolean {
        return if (other is IntTypeWithValue) value == other.value else super.equals(other)
    }

    override fun hashCode(): Int = System.identityHashCode(this)
}

class StringArrayWithValues(val values: MutableList<String?>) : Type(Const.T_ARRAY, "[Ljava/lang/String") {
    override fun toString(): String = "java.lang.String[] = $values"

    override fun equals(other: Any?): Boolean {
        return if (other is StringArrayWithValues) values == other.values else super.equals(other)
    }

    override fun hashCode(): Int = System.identityHashCode(this)
}

class StringFromArray(val array: List<String?>) : ObjectType("java.lang.String") {
    override fun toString(): String = "java.lang.String = any of $array"

    override fun equals(other: Any?): Boolean {
        return if (other is StringFromArray) array == other.array else super.equals(other)
    }

    override fun hashCode(): Int = System.identityHashCode(this)
}

class TypeWithLambda(
    private val delegate: Type,
    val method: MethodPointer,
    val args: List<Type>,
) : Type(delegate.type, delegate.signature) {
    override fun toString(): String = "$delegate with lambda: ( $method called with $args )"
}

class TypedTag(
    var nbt: NbtElement = NbtAny,
    className: String = "net.minecraft.nbt.Tag",
    var optionalUntil: Int = 0,
) : ObjectType(className) {
    override fun toString(): String = "$className = $nbt"
}

/**
 * Converts a [StackMapType] into a [Type] in the context of a [Method].
 */
fun StackMapType.asType(method: Method): Type {
    return when (type) {
        Const.ITEM_Bogus -> Type.UNKNOWN
        Const.ITEM_Integer -> Type.INT
        Const.ITEM_Float -> Type.FLOAT
        Const.ITEM_Double -> Type.DOUBLE
        Const.ITEM_Long -> Type.LONG
        Const.ITEM_Null -> Type.NULL
        // first argument in non-static methods is `this`, and `<init>` methods should be the only
        // place where this type can occur, and they aren't static.
        Const.ITEM_InitObject -> UninitializedObjectType(method.argumentTypes[0] as ObjectType)
        Const.ITEM_Object -> Type.getType(internalTypeNameToSignature(className))
        Const.ITEM_NewObject -> UninitializedObjectType(
            (InstructionList(method.code.code).toList()[index].instruction as NEW).getLoadClassType(
                ConstantPoolGen(constantPool)
            )
        )

        else -> Type.UNKNOWN
    }
}

/**
 * Re-implementation of [Type.internalTypeNameToSignature] which is package-private.
 */
fun internalTypeNameToSignature(internalTypeName: String): String {
    if (internalTypeName.isEmpty() || Const.SHORT_TYPE_NAMES.any { it == internalTypeName }) {
        return internalTypeName
    }
    return when (internalTypeName.first()) {
        '[' -> internalTypeName
        'L', 'T' -> if (internalTypeName.last() == ';') {
            internalTypeName
        } else {
            "L$internalTypeName;"
        }

        else -> "L$internalTypeName;"
    }
}

fun Type.asNbt(): NbtElement? =
    when (this) {
        is TypedTag -> nbt
        ObjectType("net.minecraft.nbt.Tag") -> NbtAny
        ObjectType("net.minecraft.nbt.ByteTag") -> NbtByte
        ObjectType("net.minecraft.nbt.ShortTag") -> NbtShort
        ObjectType("net.minecraft.nbt.IntTag") -> NbtInt
        ObjectType("net.minecraft.nbt.LongTag") -> NbtLong
        ObjectType("net.minecraft.nbt.FloatTag") -> NbtFloat
        ObjectType("net.minecraft.nbt.DoubleTag") -> NbtDouble
        ObjectType("net.minecraft.nbt.StringTag") -> NbtString
        ObjectType("net.minecraft.nbt.ByteArrayTag") -> NbtByteArray
        ObjectType("net.minecraft.nbt.IntArrayTag") -> NbtIntArray
        ObjectType("net.minecraft.nbt.LongArrayTag") -> NbtLongArray
        ObjectType("net.minecraft.nbt.ListTag") -> NbtList()
        ObjectType("net.minecraft.nbt.CompoundTag") -> NbtCompound()
        else -> null
    }

fun Type.isNbt(): Boolean = asNbt() != null

fun NbtElement.asType(): Type =
    when (this) {
        NbtAny -> ObjectType("net.minecraft.nbt.Tag")
        NbtByte -> ObjectType("net.minecraft.nbt.ByteTag")
        NbtShort -> ObjectType("net.minecraft.nbt.ShortTag")
        NbtInt -> ObjectType("net.minecraft.nbt.IntTag")
        NbtLong -> ObjectType("net.minecraft.nbt.LongTag")
        NbtFloat -> ObjectType("net.minecraft.nbt.FloatTag")
        NbtDouble -> ObjectType("net.minecraft.nbt.DoubleTag")
        NbtString -> ObjectType("net.minecraft.nbt.StringTag")
        NbtByteArray -> ObjectType("net.minecraft.nbt.ByteArrayTag")
        NbtIntArray -> ObjectType("net.minecraft.nbt.IntArrayTag")
        NbtLongArray -> ObjectType("net.minecraft.nbt.LongArrayTag")
        is NbtAnyCompound -> TypedTag(NbtCompound(unknownKeys = valueType), "net.minecraft.nbt.CompoundTag")
        is NbtEither -> TypedTag(this)
        is NbtBoxed -> TypedTag(this, "net.minecraft.nbt.CompoundTag")
        NbtNestedEntity -> TypedTag(this, "net.minecraft.nbt.CompoundTag")
        NbtBlockState -> TypedTag(this, "net.minecraft.nbt.CompoundTag")
        is NbtList -> TypedTag(this, "net.minecraft.nbt.ListTag")
        is NbtCompound -> TypedTag(this, "net.minecraft.nbt.CompoundTag")
        NbtBoolean -> TypedTag(this, "net.minecraft.nbt.ByteTag")
        NbtUuid -> TypedTag(this, "net.minecraft.nbt.IntArrayTag")
        is NbtNamedCompound -> throw IllegalStateException("named compound before finished running")
    }

fun Type.ensureTyped(): Type =
    when (this) {
        is TypedTag -> this
        ObjectType("net.minecraft.nbt.CompoundTag") -> TypedTag(NbtCompound(), className)
        ObjectType("net.minecraft.nbt.ListTag") -> TypedTag(NbtList(), className)
        ObjectType("net.minecraft.nbt.Tag") -> TypedTag()
        else -> this
    }

fun Type.untyped(): Type =
    when (this) {
        is TypedTag -> ObjectType(className)
        else -> this
    }

fun LocalVariables.toList(): List<Type> = (0..<maxLocals()).map { get(it) }

fun OperandStack.toList(): List<Type> = (0..<size()).reversed().map { peek(it) }

fun Type.forLocalsOrStack(): Type =
    when (this) {
        Type.BOOLEAN, Type.CHAR, Type.BYTE, Type.SHORT -> Type.INT
        else -> this
    }
