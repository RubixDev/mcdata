package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.*
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.*
import kotlin.math.max

class Vm(private val jarFile: String) {
    private val classes = mutableMapOf<String, JavaClass>()

    fun analyzeFrom(method: MethodPointer): NbtCompound {
        // assume a single argument of type CompoundTag
        val compound = NbtCompound()
        call(method, listOf(ObjectType(method.className), TypedCompoundTag(compound)))
        return compound
    }

    private fun call(
        methodPointer: MethodPointer,
        args: List<Type>,
        overrideOptional: Boolean = false,
    ): Type {
        val clazz = getClass(methodPointer.className)
        val method = clazz.methods.find {
            it.name == methodPointer.name
                    && Type.getMethodSignature(it.returnType, it.argumentTypes) == methodPointer.signature
        } ?: return Type.getReturnType(methodPointer.signature)

        val insns = InstructionList(method.code.code)
        val runner = MethodRunner(clazz, method, args)
        if (overrideOptional) runner.optionalUntil = Int.MAX_VALUE
        for (insn in insns) {
            runner.visit(insn)
        }

        return if (Type.getReturnType(methodPointer.signature).isNbt) {
            var nbt: NbtElement = NbtAny
            for (value in runner.returnValues) {
                nbt = value.asNbt?.merge(nbt, MergeStrategy.DifferentDataSet) ?: continue
            }
            nbt.asType
        } else {
            // TODO: or just the return type from signature?
            runner.returnValues.first()
        }
    }

    private fun getClass(name: String) = classes.getOrPut(name) {
        ClassParser(jarFile, "${name.replace('.', '/')}.class").parse()!!
    }

    /**
     * This class is responsible for "interpreting" the instructions of a method. It extends
     * [ExecutionVisitor] which already handles the stack and locals and how each instruction
     * interacts with them, and it keeps track of the [Type]s that values would have.
     *
     * Because we need to actually store some values and not just their types, we introduce
     * a few classes that extend the [Type] class but, despite the name, also hold values.
     * These are for example [StringTypeWithValue] or [TypedCompoundTag].
     */
    inner class MethodRunner(clazz: JavaClass, private val method: Method, args: List<Type>) : ExecutionVisitor() {
        private val cpg = ConstantPoolGen(clazz.constantPool)
        private val bootstrapMethods =
            clazz.attributes.filterIsInstance<BootstrapMethods>().firstOrNull()?.bootstrapMethods

        /**
         * The StackMapTable defines how the locals and stack should look at certain instructions,
         * namely all possible jump targets. This makes it possible to linearly walk through all
         * instructions.
         *
         * It is defined in a bit of a convoluted structure, so we immediately convert it into a
         * simple map of [pc] values to the expected state of locals and the stack at that location.
         */
        private val stackMapTable = method.code.stackMap?.stackMap?.let { entries ->
            val out = mutableMapOf<Int, Pair<List<Type>, List<Type>>>()
            var offset = -1
            // the implicit 0th frame uses the method arguments as locals and has an empty stack
            var prevFrame = args to listOf<Type>()
            for (entry in entries) {
                offset += entry.byteCodeOffset + 1
                // each frame is defined as a diff to the previous one
                val frame = when (entry.frameType) {
                    in Const.SAME_FRAME..Const.SAME_FRAME_MAX, Const.SAME_FRAME_EXTENDED -> {
                        // keep locals, empty stack
                        prevFrame.first to listOf()
                    }

                    in Const.SAME_LOCALS_1_STACK_ITEM_FRAME..Const.SAME_LOCALS_1_STACK_ITEM_FRAME_MAX,
                    Const.SAME_LOCALS_1_STACK_ITEM_FRAME_EXTENDED -> {
                        // keep locals, stack with one item
                        prevFrame.first to listOf(entry.typesOfStackItems.first().asType(method))
                    }

                    in Const.CHOP_FRAME..Const.CHOP_FRAME_MAX -> {
                        // chop the last k locals, empty stack
                        val k = 251 - entry.frameType
                        prevFrame.first.subList(0, prevFrame.first.size - k) to listOf()
                    }

                    in Const.APPEND_FRAME..Const.APPEND_FRAME_MAX -> {
                        // add k locals, empty stack
                        val k = entry.frameType - 251
                        Pair(
                            prevFrame.first + entry.typesOfLocals.take(k).flatMap {
                                when (val type = it.asType(method)) {
                                    Type.LONG, Type.DOUBLE -> sequenceOf(type, Type.UNKNOWN)
                                    else -> sequenceOf(type)
                                }
                            },
                            listOf()
                        )
                    }

                    Const.FULL_FRAME -> {
                        // given locals and stack
                        Pair(
                            entry.typesOfLocals.flatMap {
                                when (val type = it.asType(method)) {
                                    Type.LONG, Type.DOUBLE -> sequenceOf(type, Type.UNKNOWN)
                                    else -> sequenceOf(type)
                                }
                            },
                            entry.typesOfStackItems.map { it.asType(method) },
                        )
                    }

                    else -> throw IllegalStateException("invalid frame type: ${entry.frameType}")
                }
                out[offset] = frame
                prevFrame = frame
            }
            out
        } ?: mapOf()

        /**
         * The StackMapTable is awesome for allowing linear traversal of the instructions. However, it only
         * defines the _types_ of the locals and stack at any jump target. We also use the [Type]s to store
         * extra information we gathered though, such as constant String values and the precise NBT structure
         * of the NBT tags. This information would be lost if we simply took all types from the StackMapTable
         * and used them as values. So in an attempt to preserve this info as often as possible while still only
         * using linear traversal, we also store the state of locals and the stack for every branch to a future
         * instruction that we encounter. That way we also store the extra information that we had stored in them.
         * We can then combine these states with the info from the StackMapTable to guarantee the correct types
         * but also keep the existing [Type] instance if there is one.
         *
         * This approach is not perfect, but still much simpler than properly interpreting the instructions and
         * jumps, as in that case we'd have to try out _every_ possible path across all jumps (two choices per
         * branching instruction) and would also have to deal with things like loops.
         */
        private val extraStackMap = mutableMapOf<Int, Pair<List<Type>, List<Type>>>()

        private val frame = Frame(method.code.maxLocals, method.code.maxStack)
        private val locals get() = frame.locals
        private val stack get() = frame.stack

        private var pc = 0
        var optionalUntil = 0

        /**
         * Because we run all instructions in linear order, we can encounter multiple return instructions.
         * This list stores the value from each of those. They then get combined into one later on as they
         * should all describe the same structure, but some of them might hold more information than others.
         */
        var returnValues = mutableListOf<Type>()

        init {
            setConstantPoolGen(cpg)
            setFrame(frame)
            for ((idx, arg) in args.withIndex()) {
                locals[idx] = arg
            }
            for (idx in args.size..<locals.maxLocals()) {
                locals[idx] = UninitializedLocal
            }
        }

        fun visit(o: InstructionHandle) {
            pc = o.position

            // update locals and stack based on StackMapTable.
            // This is needed because we linearly walk through the instructions and don't perform jumps, so the state
            // of the locals and stack can be incorrect at jump targets. The StackMapTable defines how the frame should
            // look at each of those points to make linear traversal possible.
            stackMapTable[pc]?.let { entry ->
                // replace all locals
                for ((i, newType) in entry.first.withIndex()) {
                    val extraType = extraStackMap[pc]?.first?.getOrNull(i)
                    val prevType = locals[i]
                    // use types from `extraStackMap` or the previous locals if possible, to not lose info attached to them
                    locals[i] = when {
                        extraType != null && extraType.className == newType.className -> extraType
                        prevType.className == newType.className -> prevType
                        else -> newType
                    }
                }
                // set the rest to uninitialized
                for (i in entry.first.size..<locals.maxLocals()) {
                    locals[i] = UninitializedLocal
                }

                // replace the stack
                val prev = stack.toList()
                stack.clear()
                for ((i, newType) in entry.second.withIndex()) {
                    val extraType = extraStackMap[pc]?.second?.getOrNull(i)
                    val prevType = prev.getOrNull(i)
                    // use types from `extraStackMap` or the previous locals if possible, to not lose info attached to them
                    stack.push(
                        when {
                            extraType != null && extraType.className == newType.className -> extraType
                            prevType != null && prevType.className == newType.className -> prevType
                            else -> newType
                        }
                    )
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
                    "put" -> stack.peek().asNbt
                    else -> null
                }
                val compound = stack.peek(2)
                if (type != null) {
                    if (compound !is TypedCompoundTag) {
                        println("WARNING: untyped CompoundTag on stack, cannot save value")
                    } else {
                        compound.nbt.put(
                            (stack.peek(1) as StringTypeWithValue).value,
                            NbtCompoundEntry(
                                type,
                                optional = pc < optionalUntil
                            )
                        )
                    }
                }
            } else if (o.getClassName(cpg) == "net.minecraft.nbt.ListTag") {
                val type = when (o.getMethodName(cpg)) {
                    // both `add(index: Int, element: Tag)` and `add(element: Tag)` have the tag as their last argument
                    "add", "addTag", "addFirst", "addLast" -> stack.peek().asNbt
                    // TODO: `addAll`
                    "addAll" -> println("INFO: called `addAll` on ListTag").let { null }
                    "set", "setTag" -> stack.peek().asNbt
                    // TODO: can we get info from `get()` and `remove()`?
                    "getCompound" -> NbtCompound()
                    "getList" -> NbtList()
                    "getShort" -> NbtShort
                    "getInt" -> NbtInt
                    "getIntArray" -> NbtIntArray
                    "getLongArray" -> NbtLongArray
                    "getDouble" -> NbtDouble
                    "getFloat" -> NbtFloat
                    "getString" -> NbtString
                    else -> null
                }
                val argCount = Type.getArgumentTypes(o.getSignature(cpg)).size
                val list = stack.peek(argCount)
                if (type != null) {
                    if (list !is TypedListTag) {
                        println("WARNING: untyped ListTag on stack, cannot save value")
                    } else {
                        list.nbt.add(type)
                    }
                }
            } else if (o.getClassName(cpg) == "java.util.Optional" && o.getMethodName(cpg) == "ifPresent") {
                // lambda methods are resolved in `visitINVOKEDYNAMIC`, now actually call
                // it but mark everything as optional because it's in `Optional.ifPresent`
                val consumer = stack.peek()
                if (consumer is TypeWithLambda) {
                    call(
                        consumer.method,
                        consumer.args,
                        overrideOptional = true,
                    )
                }
            }
            // TODO: call most other methods to find out more about nested types
            super.visitINVOKEVIRTUAL(o)
        }

        override fun visitINVOKEDYNAMIC(o: INVOKEDYNAMIC) {
            val bootstrapArgs = o.getArgumentTypes(cpg).indices.reversed().map { stack.peek(it) }
            super.visitINVOKEDYNAMIC(o)
            bootstrapMethods ?: return
            // we don't actually do dynamic invocation and construction of CallSite and so on. We only care about
            // which static synthetic method backs the lambda logic and want to "call" that directly.
            // Which method that is, is passed to the bootstrap method in the second argument.
            val cp = cpg.constantPool
            // get the arguments
            val bootstrapMethod =
                bootstrapMethods[cp.getConstant<ConstantInvokeDynamic>(o.index).bootstrapMethodAttrIndex]
            // the handle to the backing method
            val lambdaMethodHandle = cp.getConstant<ConstantMethodHandle>(bootstrapMethod.bootstrapArguments[1])
            // the lambda's signature
            val lambdaSignature =
                cp.getConstantUtf8(cp.getConstant<ConstantMethodType>(bootstrapMethod.bootstrapArguments[2]).descriptorIndex).bytes
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
                val lambdaMethodSignature = lambdaMethodNameAndType.getSignature(cp)
                // attach info about this lambda to the top stack value
                stack.push(
                    TypeWithLambda(
                        delegate = stack.pop(),
                        method = MethodPointer(
                            className = lambdaMethodClass,
                            name = lambdaMethodName,
                            signature = lambdaMethodSignature,
                        ),
                        // TODO: maybe try to somehow pass the correct actual values here?
                        //  would require special casing for common stuff like Optional unless we want to
                        //  properly interpret actual java code
                        args = bootstrapArgs + Type.getArgumentTypes(lambdaSignature),
                    )
                )
            }
        }

        override fun visitLDC(o: LDC) {
            // save the strings value for later use
            val c = cpg.getConstant(o.index)
            if (c is ConstantString) {
                stack.push(StringTypeWithValue(cpg.constantPool.getConstantUtf8(c.stringIndex).bytes))
            } else {
                super.visitLDC(o)
            }
        }

        override fun visitLDC_W(o: LDC_W) {
            // save the strings value for later use
            val c = cpg.getConstant(o.index)
            if (c is ConstantString) {
                stack.push(StringTypeWithValue(cpg.constantPool.getConstantUtf8(c.stringIndex).bytes))
            } else {
                super.visitLDC(o)
            }
        }

        // `visitBranchInstruction` already exists, but it's called _before_ the individual
        // methods which manipulate the stack, but we need it _after_
        private fun visitBranchInstructionAfter(o: BranchInstruction) {
            optionalUntil = max(o.target.position, optionalUntil)
            if (o.target.position > pc) {
                extraStackMap[o.target.position] = locals.toList() to stack.toList()
            }
        }

        override fun visitGOTO(o: GOTO) {
            super.visitGOTO(o); visitBranchInstructionAfter(o)
        }

        override fun visitGOTO_W(o: GOTO_W) {
            super.visitGOTO_W(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFEQ(o: IFEQ) {
            super.visitIFEQ(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFGE(o: IFGE) {
            super.visitIFGE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFGT(o: IFGT) {
            super.visitIFGT(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFLE(o: IFLE) {
            super.visitIFLE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFLT(o: IFLT) {
            super.visitIFLT(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFNE(o: IFNE) {
            super.visitIFNE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFNONNULL(o: IFNONNULL) {
            super.visitIFNONNULL(o); visitBranchInstructionAfter(o)
        }

        override fun visitIFNULL(o: IFNULL) {
            super.visitIFNULL(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ACMPEQ(o: IF_ACMPEQ) {
            super.visitIF_ACMPEQ(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ACMPNE(o: IF_ACMPNE) {
            super.visitIF_ACMPNE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPEQ(o: IF_ICMPEQ) {
            super.visitIF_ICMPEQ(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPGE(o: IF_ICMPGE) {
            super.visitIF_ICMPGE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPGT(o: IF_ICMPGT) {
            super.visitIF_ICMPGT(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPLE(o: IF_ICMPLE) {
            super.visitIF_ICMPLE(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPLT(o: IF_ICMPLT) {
            super.visitIF_ICMPLT(o); visitBranchInstructionAfter(o)
        }

        override fun visitIF_ICMPNE(o: IF_ICMPNE) {
            super.visitIF_ICMPNE(o); visitBranchInstructionAfter(o)
        }

        override fun visitJSR(o: JSR) {
            super.visitJSR(o); visitBranchInstructionAfter(o)
        }

        override fun visitJSR_W(o: JSR_W) {
            super.visitJSR_W(o); visitBranchInstructionAfter(o)
        }

        override fun visitLOOKUPSWITCH(o: LOOKUPSWITCH) {
            super.visitLOOKUPSWITCH(o); visitBranchInstructionAfter(o)
        }

        override fun visitTABLESWITCH(o: TABLESWITCH) {
            super.visitTABLESWITCH(o); visitBranchInstructionAfter(o)
        }

        override fun visitReturnInstruction(o: ReturnInstruction) {
            if (o is RETURN) {
                returnValues.add(Type.VOID)
            } else {
                returnValues.add(stack.peek())
            }
        }

        override fun visitASTORE(o: ASTORE) {
            // when storing NBT types in the locals, make sure they have type
            // information attached which can then later be expanded
            val type = when (stack.peek()) {
                is TypedCompoundTag,
                is TypedListTag,
                is TypedTag -> return super.visitASTORE(o)
                ObjectType("net.minecraft.nbt.CompoundTag") -> TypedCompoundTag()
                ObjectType("net.minecraft.nbt.ListTag") -> TypedListTag()
                ObjectType("net.minecraft.nbt.Tag") -> TypedTag()
                else -> return super.visitASTORE(o)
            }
            stack.pop()
            locals[o.index] = type
        }
    }
}
