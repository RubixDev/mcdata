package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.Method
import org.apache.bcel.classfile.StackMapType
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.LocalVariables
import org.apache.bcel.verifier.structurals.OperandStack
import org.apache.bcel.verifier.structurals.UninitializedObjectType

object UninitializedLocal : Type(Const.T_UNKNOWN, "<uninitialized local>")

data class MethodPointer(
    val className: String,
    val name: String,
    val signature: String,
) {
    override fun toString(): String = "$className.$name$signature"
}

class StringTypeWithValue(val value: String) : ObjectType("java.lang.String") {
    override fun toString(): String = "java.lang.String = \"$value\""
}

class TypeWithLambda(
    private val delegate: Type,
    val method: MethodPointer,
    val args: List<Type>,
) : Type(delegate.type, delegate.signature) {
    override fun toString(): String = "$delegate with lambda: ( $method called with $args )"
}

class TypedCompoundTag(val nbt: NbtCompound = NbtCompound()) : ObjectType("net.minecraft.nbt.CompoundTag") {
    override fun toString(): String = "net.minecraft.nbt.CompoundTag = $nbt"
}

class TypedListTag(val nbt: NbtList = NbtList()) : ObjectType("net.minecraft.nbt.ListTag") {
    override fun toString(): String = "net.minecraft.nbt.ListTag = $nbt"
}

class TypedTag(val nbt: NbtElement = NbtAny, className: String = "net.minecraft.nbt.Tag") : ObjectType(className) {
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

val Type.asNbt: NbtElement?
    get() = when (this) {
        is TypedCompoundTag -> nbt
        is TypedListTag -> nbt
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

val Type.isNbt: Boolean get() = asNbt != null

val NbtElement.asType: Type
    get() = when (this) {
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
        NbtAnyCompound -> ObjectType("net.minecraft.nbt.CompoundTag")
        is NbtList -> TypedListTag(this)
        is NbtCompound -> TypedCompoundTag(this)
        NbtBoolean -> TypedTag(this, "net.minecraft.nbt.ByteTag")
        NbtUuid -> TypedTag(this, "net.minecraft.nbt.IntArrayTag")
        is NbtNamedCompound -> throw IllegalStateException("named compound before finished running")
    }

fun LocalVariables.toList(): List<Type> = (0..<maxLocals()).map { get(it) }

fun OperandStack.toList(): List<Type> = (0..<size()).reversed().map { peek(it) }
