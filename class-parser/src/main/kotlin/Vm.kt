package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.*
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.ExecutionVisitor
import org.apache.bcel.verifier.structurals.Frame
import org.apache.bcel.verifier.structurals.UninitializedObjectType
import kotlin.math.max

class Vm(private val jarFile: String) {
    private val classes = mutableMapOf<String, JavaClass>()

    fun call(className: String, methodName: String, methodDescriptor: String): NbtCompound {
//        println(className)
        val clazz = getClass(className)
//        for (method in clazz.methods) {
//            println("${method.name}${Type.getMethodSignature(method.returnType, method.argumentTypes)} ${method.signature}")
//        }
        val method = clazz.methods.find {
            it.name == methodName && Type.getMethodSignature(it.returnType, it.argumentTypes) == methodDescriptor
        } ?: return NbtCompound(mutableMapOf())

//        println(method.code)

        val insns = InstructionList(method.code.code)
        val runner = MethodRunner(clazz, method)
        for (insn in insns) {
            runner.visit(insn)
        }

        // TODO: find out whether super is called
        return runner.nbt
    }

    private fun getClass(name: String) = classes.getOrPut(name) {
        ClassParser(jarFile, "${name.replace('.', '/')}.class").parse()!!
    }

    inner class MethodRunner(clazz: JavaClass, private val method: Method) : ExecutionVisitor() {
        private val cpg = ConstantPoolGen(clazz.constantPool)
        private val bootstrapMethods = clazz.attributes.filterIsInstance<BootstrapMethods>().firstOrNull()?.bootstrapMethods
        private val stackMapTable = method.code.stackMap?.stackMap ?: arrayOf()

        private val frame = Frame(method.code.maxLocals, method.code.maxStack)
        private val locals get() = frame.locals
        private val stack get() = frame.stack

        private var pc = 0
        private var optionalUntil = 0

        val nbt = NbtCompound(mutableMapOf())

        init {
            setConstantPoolGen(cpg)
            setFrame(frame)
        }

        fun visit(o: InstructionHandle) {
            pc = o.position

            // update locals and stack based on StackMapTable.
            // This is needed because we linearly walk through the instructions and don't perform jumps, so the state
            // of the locals and stack can be incorrect at jump targets. The StackMapTable defines how the frame should
            // look at each of those points to make linear traversal possible.
            var offset = -1
            for (entry in stackMapTable) {
                offset += entry.byteCodeOffset + 1
                if (pc == offset) {
                    // TODO: do we really have to clear the stack in all of these?
                    when (entry.frameType) {
                        in Const.SAME_FRAME..Const.SAME_FRAME_MAX, Const.SAME_FRAME_EXTENDED -> {
                            // keep locals, clear the stack
                            stack.clear()
                        }

                        in Const.SAME_LOCALS_1_STACK_ITEM_FRAME..Const.SAME_LOCALS_1_STACK_ITEM_FRAME_MAX,
                        Const.SAME_LOCALS_1_STACK_ITEM_FRAME_EXTENDED -> {
                            // keep locals, replace the stack with one entry
                            stack.clear()
                            stack.push(entry.typesOfStackItems[0].asType(method))
                        }

                        in Const.CHOP_FRAME..Const.CHOP_FRAME_MAX -> {
                            // chop some locals
//                            println(locals)
                            val k = 251 - entry.frameType
//                            println("k = $k")
                            // TODO: is this correct? should it instead be based on maxLocals? is it possible that there is
                            //  an unknown local before the last local?
                            val currentLocalsLen = if (locals[locals.maxLocals() - 1] != Type.UNKNOWN) {
                                locals.maxLocals()
                            } else {
                                (0..<locals.maxLocals()).map { locals[it] }.dropLastWhile { it == Type.UNKNOWN }.size
                            }
                            for (i in locals.maxLocals() - k..<locals.maxLocals()) {
                                locals[i] = Type.UNKNOWN
                            }

                            // clear the stack
                            stack.clear()
                        }

                        in Const.APPEND_FRAME..Const.APPEND_FRAME_MAX -> {
                            // add some locals
//                            println(locals)
                            val k = entry.frameType - 251
//                            println("k = $k")
                            require(entry.typesOfLocals.size == entry.numberOfLocals)
                            require(entry.numberOfLocals == k)
                            // TODO: is this correct? should it instead be based on maxLocals? is it possible that there is
                            //  an unknown local before the last local?
//                            val currentLocalsLen = if (locals[locals.maxLocals() - 1] != Type.UNKNOWN) {
//                                locals.maxLocals()
//                            } else {
//                                (0..<locals.maxLocals()).map { locals[it] }.dropLastWhile { it == Type.UNKNOWN }.size
//                            }
//                            for ((i, type) in (currentLocalsLen..<currentLocalsLen + k).zip(entry.typesOfLocals)) {
//                                locals[i] = type.asType(method)
//                            }

                            // clear the stack
                            stack.clear()
                        }

                        Const.FULL_FRAME -> {
                            // replace all locals
                            for ((i, type) in entry.typesOfLocals.withIndex()) {
                                locals[i] = type.asType(method)
                            }
                            for (i in entry.typesOfLocals.size..<locals.maxLocals()) {
                                locals[i] = Type.UNKNOWN
                            }

                            // replace the stack
                            val prev = (0..<stack.size()).reversed().map { stack.peek(it) }
                            stack.clear()
                            for ((i, sType) in entry.typesOfStackItems.withIndex()) {
                                val type = sType.asType(method)
                                if (type == Type.STRING && prev[i] is StringTypeWithValue) {
                                    // keep string values if they were known at the same position before
                                    stack.push(prev[i])
                                } else {
                                    stack.push(type)
                                }
                            }
                        }
                    }
                }
            }

            o.accept(this)
        }

        override fun visitINVOKEVIRTUAL(o: INVOKEVIRTUAL) {
            if (o.getClassName(cpg) == "net.minecraft.nbt.CompoundTag") {
                // TODO: same for the getX methods
                val type = when (o.getMethodName(cpg)) {
                    "putByte" -> NbtByte
                    "putShort" -> NbtShort
                    "putInt" -> NbtInt
                    "putLong" -> NbtLong
                    "putUUID" -> NbtUuid
                    "putFloat" -> NbtFloat
                    "putDouble" -> NbtDouble
                    "putString" -> NbtString
                    "putByteArray" -> NbtByteArray
                    "putIntArray" -> NbtIntArray
                    "putLongArray" -> NbtLongArray
                    "putBoolean" -> NbtBoolean
                    "put" -> {
                        // TODO: find out inner types
                        when (stack.peek().className) {
                            "net.minecraft.nbt.ByteTag" -> NbtByte
                            "net.minecraft.nbt.ShortTag" -> NbtShort
                            "net.minecraft.nbt.IntTag" -> NbtInt
                            "net.minecraft.nbt.LongTag" -> NbtLong
                            "net.minecraft.nbt.FloatTag" -> NbtFloat
                            "net.minecraft.nbt.DoubleTag" -> NbtDouble
                            "net.minecraft.nbt.StringTag" -> NbtString
                            "net.minecraft.nbt.ByteArrayTag" -> NbtByteArray
                            "net.minecraft.nbt.IntArrayTag" -> NbtIntArray
                            "net.minecraft.nbt.LongArrayTag" -> NbtLongArray
                            "net.minecraft.nbt.ListTag" -> NbtList(NbtAny)
                            "net.minecraft.nbt.CompoundTag" -> NbtAnyCompound
                            else -> NbtAny
                        }
                    }

                    else -> null
                }
                if (type != null) {
                    nbt.add(
                        (stack.peek(1) as StringTypeWithValue).value,
                        NbtCompoundEntry(
                            type,
                            optional = pc < optionalUntil
                        )
                    )
                }
            } else if (o.getClassName(cpg) == "java.util.Optional" && o.getMethodName(cpg) == "ifPresent") {
                // lambda methods in `Optional.ifPresent` are "called" (in `visitINVOKEDYNAMIC`) and all their entries
                // are added and marked optional
                val consumer = stack.peek()
                if (consumer is TypeWithNbt) {
                    consumer.nbt.entries.onEach { it.value.optional = true }.forEach { nbt.add(it.key, it.value) }
                }
            }
            // TODO: call most other methods to find out more about nested types
            super.visitINVOKEVIRTUAL(o)
        }

        override fun visitINVOKEDYNAMIC(o: INVOKEDYNAMIC) {
            super.visitINVOKEDYNAMIC(o)
            bootstrapMethods ?: return
            // we don't actually do dynamic invocation and construction of CallSite and so on. We only care about
            // which static synthetic method backs the lambda logic and want to "call" that directly.
            // Which method that is, is passed to the bootstrap method in the second argument.
            val cp = cpg.constantPool
            // get the argument
            val lambdaMethodHandle = cp.getConstant<ConstantMethodHandle>(
                bootstrapMethods[cp.getConstant<ConstantInvokeDynamic>(o.index).bootstrapMethodAttrIndex].bootstrapArguments[1],
            )
            // ignore other kinds of dynamic invocation
            if (lambdaMethodHandle.referenceKind == Const.REF_invokeStatic.toInt()) {
                // get the method reference
                val lambdaMethod = cp.getConstant<ConstantMethodref>(lambdaMethodHandle.referenceIndex)
                // get the name of the class that contains the method
                val lambdaMethodClass =
                    cp.getConstantUtf8(cp.getConstant<ConstantClass>(lambdaMethod.classIndex).nameIndex).bytes
                // get the method name and descriptor
                val lambdaMethodNameAndType = cp.getConstant<ConstantNameAndType>(lambdaMethod.nameAndTypeIndex)
                val lambdaMethodName = lambdaMethodNameAndType.getName(cp)
                val lambdaMethodDescriptor = lambdaMethodNameAndType.getSignature(cp)
                // "call" that method and attach the resulting NBT to the top stack value
                val nbt = call(lambdaMethodClass, lambdaMethodName, lambdaMethodDescriptor)
                stack.push(TypeWithNbt(stack.pop(), nbt))
            }
        }

        override fun visitLDC(o: LDC) {
            // also remember the value of constant strings, not just the type, for later use
            val c = cpg.getConstant(o.index)
            if (c is ConstantString) {
                stack.push(StringTypeWithValue(cpg.constantPool.getConstantUtf8(c.stringIndex).bytes))
            } else {
                super.visitLDC(o)
            }
        }

        override fun visitLDC_W(o: LDC_W) {
            // also remember the value of constant strings, not just the type, for later use
            val c = cpg.getConstant(o.index)
            if (c is ConstantString) {
                stack.push(StringTypeWithValue(cpg.constantPool.getConstantUtf8(c.stringIndex).bytes))
            } else {
                super.visitLDC(o)
            }
        }

        override fun visitBranchInstruction(o: BranchInstruction) {
            optionalUntil = max(o.target.position, optionalUntil)
        }
    }
}

class StringTypeWithValue(val value: String) : ObjectType("java.lang.String") {
    override fun toString(): String = "java.lang.String = \"$value\""
}

class TypeWithNbt(private val delegate: Type, val nbt: NbtCompound) : Type(delegate.type, delegate.signature) {
    override fun toString(): String = "$delegate with NBT: $nbt"
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
        Const.ITEM_Object -> ObjectType(className)
        Const.ITEM_NewObject -> UninitializedObjectType(
            (InstructionList(method.code.code).toList()[index].instruction as NEW).getLoadClassType(
                ConstantPoolGen(constantPool)
            )
        )

        else -> Type.UNKNOWN
    }
}
