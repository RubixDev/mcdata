package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.*
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.ExecutionVisitor
import org.apache.bcel.verifier.structurals.Frame
import java.io.IOException
import kotlin.math.max

class Vm(private val jarFile: String) {
    private val classes = mutableMapOf<String, JavaClass>()
    private val callStack = mutableListOf<MethodPointer>()

    fun analyzeFrom(method: MethodPointer): NbtCompound {
        // assume a single argument of type CompoundTag
        val compound = NbtCompound()
        call(method, listOf(ObjectType(method.className), TypedCompoundTag(compound)), ignoreSuper = true)
        return compound
    }

    private fun call(
        methodPointer: MethodPointer,
        args: List<Type>,
        overrideOptional: Boolean = false,
        ignoreSuper: Boolean = false,
    ): Type {
        // no recursion
        // TODO: recursive structures, e.g. MobEffectInstance
        if (callStack.contains(methodPointer)) {
            return Type.getReturnType(methodPointer.signature).ensureTyped()
        }

        val clazz = getClass(methodPointer.className)
        val method = clazz.methods.find {
            it.name == methodPointer.name
                    && Type.getMethodSignature(it.returnType, it.argumentTypes) == methodPointer.signature
        } ?: return Type.getReturnType(methodPointer.signature)

        callStack.add(methodPointer)

        // if there's only one CompoundTag argument, and it doesn't have a name yet, give it one based
        // on the class and method names. The caller may add additional fields to the passed tag, in
        // which case a distinct number will later be appended to this name for each distinct caller.
        val compoundArgs = args.filterIsInstance<TypedCompoundTag>()
        if (compoundArgs.size == 1) {
            val tag = compoundArgs.first().nbt
            if (tag.name == null) {
                tag.name = "${classToTypeName(methodPointer.className)}_${methodPointer.name}"
            }
        }

        val insns = InstructionList(method.code.code)
        val runner = MethodRunner(clazz, method, args, ignoreSuper)
        if (overrideOptional) runner.optionalUntil = Int.MAX_VALUE
        for (insn in insns) {
            runner.visit(insn)
        }
        callStack.removeLast()

        val returnType = Type.getReturnType(methodPointer.signature)
        return if (returnType == Type.VOID) {
            Type.VOID
        } else if (returnType.isNbt()) {
            var nbt: NbtElement = NbtAny
            for (value in runner.returnValues) {
                nbt = value.asNbt()?.merge(nbt, MergeStrategy.DifferentDataSet) ?: continue
            }
            // if the return value is a CompoundTag, and it doesn't have a name yet, give it one based
            // on the class and method names
            if (nbt is NbtCompound && nbt.name == null) {
                nbt.name = "${classToTypeName(methodPointer.className)}_${methodPointer.name}"
            }
            nbt.asType()
        } else {
            runner.returnValues.first()
        }
    }

    private fun getClass(name: String) = classes.getOrPut(name) {
        ClassParser(jarFile, "${name.replace('.', '/')}.class").parse()
    }

    private fun getClassOrNull(name: String): JavaClass? {
        return classes.getOrPut(name) {
            try {
                ClassParser(jarFile, "${name.replace('.', '/')}.class").parse()
            } catch (_: IOException) {
                return null
            }
        }
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
    inner class MethodRunner(
        clazz: JavaClass,
        private val method: Method, args: List<Type>,
        private val ignoreSuper: Boolean,
    ) : ExecutionVisitor() {
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
                locals[idx] = arg.ensureTyped()
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
                    }.ensureTyped()
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
            val className = o.getClassName(cpg)
            val methodName = o.getMethodName(cpg)
            val signature = o.getSignature(cpg)
            val argTypes = Type.getArgumentTypes(signature)
            val returnType = Type.getReturnType(signature)

            if (className == "net.minecraft.nbt.CompoundTag") {
                // TODO: same for the getX methods
                val type = when (methodName) {
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
                    "put" -> stack.peek().asNbt()
                    else -> null
                }
                if (type != null) {
                    val compound = stack.peek(2)
                    if (compound !is TypedCompoundTag) {
                        println("WARNING: untyped CompoundTag on stack, cannot save value")
                    } else {
                        when (val string = stack.peek(1) as? StringTypeWithValue) {
                            null -> compound.nbt.unknownKeys = type.encompass(compound.nbt.unknownKeys)
                            else -> compound.nbt.put(
                                string.value,
                                NbtCompoundEntry(
                                    type,
                                    optional = pc < optionalUntil
                                )
                            )
                        }
                    }
                }
            } else if (className == "net.minecraft.nbt.ListTag") {
                val type = when (methodName) {
                    // both `add(index: Int, element: Tag)` and `add(element: Tag)` have the tag as their last argument
                    "add", "addTag", "addFirst", "addLast" -> stack.peek().asNbt()
                    // TODO: `addAll`
                    "addAll" -> println("INFO: called `addAll` on ListTag").let { null }
                    "set", "setTag" -> stack.peek().asNbt()
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
                val list = stack.peek(argTypes.size)
                if (type != null) {
                    if (list !is TypedListTag) {
                        println("WARNING: untyped ListTag on stack, cannot save value")
                    } else {
                        list.nbt.add(type)
                    }
                }
            } else if (className == "java.util.Optional" && methodName == "ifPresent") {
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
            } else if (invoke(className, methodName, signature, argTypes, returnType, virtual = true, static = false)) {
                return
            }
            // further candidates for special cases:
            // - Either.map - call both lambdas and mark all as optional
            // - DataResult.resultOrPartial
            //   - Codecs with NbtOps
            // TODO: the default visitINVOKE* impls replace bools, chars, bytes, and shorts with int. we might not want that
            super.visitINVOKEVIRTUAL(o)
        }

        override fun visitINVOKEINTERFACE(o: INVOKEINTERFACE) {
            if (!invoke(
                    o.getClassName(cpg),
                    o.getMethodName(cpg),
                    o.getSignature(cpg),
                    virtual = true,
                    static = false,
                )
            ) {
                super.visitINVOKEINTERFACE(o)
            }
        }

        override fun visitINVOKESTATIC(o: INVOKESTATIC) {
            if (
                !invoke(
                    o.getClassName(cpg),
                    o.getMethodName(cpg),
                    o.getSignature(cpg),
                    virtual = false,
                    static = true,
                )
            ) {
                super.visitINVOKESTATIC(o)
            }
        }

        override fun visitINVOKESPECIAL(o: INVOKESPECIAL) {
            val methodName = o.getMethodName(cpg)
            val signature = o.getSignature(cpg)
            if (
                (ignoreSuper && methodName == method.name && signature == method.signature)
                || !invoke(
                    o.getClassName(cpg),
                    methodName,
                    signature,
                    virtual = false,
                    static = false,
                )
            ) {
                super.visitINVOKESPECIAL(o)
            }
        }

        private fun invoke(
            className: String,
            methodName: String,
            signature: String,
            virtual: Boolean,
            static: Boolean,
        ) = invoke(
            className,
            methodName,
            signature,
            Type.getArgumentTypes(signature),
            Type.getReturnType(signature),
            virtual,
            static,
        )

        /**
         * Resolve and call the specified method.
         * @return `true` if the stack got updated, `false` if nothing was done.
         */
        private fun invoke(
            className: String,
            methodName: String,
            signature: String,
            argTypes: Array<Type>,
            returnType: Type,
            virtual: Boolean,
            static: Boolean,
        ): Boolean {
            // special case for `Entity.saveWithoutId`:
            // we never want to re-enter that method as that would introduce a recursive structure,
            // which we mark by setting `nestedEntity` to `true`
            if (
                className == "net.minecraft.world.entity.Entity"
                && methodName == "saveWithoutId"
                && signature == "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;"
            ) {
                val args = (0..1).map { stack.pop() }.reversed()
                (args[1] as TypedCompoundTag).nbt.nestedEntity = true
                stack.push(args[1])
                return true
            }

            // enter every method that takes or returns any NBT type for further analysis
            if (argTypes.any { it.isNbt() } || returnType.isNbt()) {
                // get the args from the stack, including the instance if not static
                val args = (0..<argTypes.size + if (static) 0 else 1).reversed().map { stack.peek(it) }
                // resolve the method to call
                val targetClass = when (static || !virtual) {
                    true -> className
                    false -> args.first().className.also {
                        // TODO: is this guaranteed? which one do we use?
                        require(it == className) { "$it != $className" }
                    }
                }
                val targetMethod = when (virtual) {
                    true -> resolveVirtual(MethodPointer(targetClass, methodName, signature)) ?: return false
                    false -> MethodPointer(targetClass, methodName, signature)
                }
                // pop the args
                stack.pop(args.size)
                // call it
                val res = call(targetMethod, args)
                // store the result
                if (returnType != Type.VOID) {
                    stack.push(res)
                }
                return true
            }
            return false
        }

        /**
         * Resolves a virtual method as described in the [Java Virtual Machine Specification](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-6.html#jvms-6.5.invokevirtual).
         */
        private fun resolveVirtual(method: MethodPointer): MethodPointer? {
            // Let C be the class of objectref. The actual method to be invoked is selected by the following lookup
            // procedure:
            // 1. If C contains a declaration for an instance method m that overrides (§5.4.5) the resolved method,
            //    then m is the method to be invoked.
            var clazz = getClass(method.className)
            if (clazz.methods.any { it.name == method.name && it.signature == method.signature && !it.isAbstract }) {
                return method
            }
            // 2. Otherwise, if C has a superclass, a search for a declaration of an instance method that overrides
            //    the resolved method is performed, starting with the direct superclass of C and continuing with the
            //    direct superclass of that class, and so forth, until an overriding method is found or no further
            //    superclasses exist. If an overriding method is found, it is the method to be invoked.
            val superInterfaceNames = mutableListOf<String>()
            while (clazz.className != Type.OBJECT.className) {
                superInterfaceNames.addAll(clazz.interfaceNames)
                clazz = getClassOrNull(clazz.superclassName) ?: break
                if (clazz.methods.any { it.name == method.name && it.signature == method.signature && !it.isAbstract }) {
                    return MethodPointer(clazz.className, method.name, method.signature)
                }
            }
            // 3. Otherwise, if there is exactly one maximally-specific method (§5.4.3.3) in the superinterfaces of
            //    C that matches the resolved method's name and descriptor and is not abstract, then it is the
            //    method to be invoked.
            val superInterFaces = superInterfaceNames.mapNotNull { getClassOrNull(it) }
            val candidates = superInterFaces.mapNotNull { iface ->
                val m =
                    iface.methods.find { it.name == method.name && it.signature == method.signature && !it.isAbstract }
                when (m) {
                    null -> null
                    else -> MethodPointer(iface.className, m.name, m.signature)
                }
            }
            if (candidates.size == 1) {
                return candidates.first()
            }

            // if we try to call an abstract method on an abstract class there's no code to execute, so we can skip
            // this invoke
            clazz = getClass(method.className)
            if (clazz.isAbstract && clazz.methods.any { it.name == method.name && it.signature == method.signature && it.isAbstract }) {
                return null
            }
            throw RuntimeException("Virtual method $method could not be resolved")
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

        // @formatter:off
        override fun visitGOTO(o: GOTO) { super.visitGOTO(o); visitBranchInstructionAfter(o) }
        override fun visitGOTO_W(o: GOTO_W) { super.visitGOTO_W(o); visitBranchInstructionAfter(o) }
        override fun visitIFEQ(o: IFEQ) { super.visitIFEQ(o); visitBranchInstructionAfter(o) }
        override fun visitIFGE(o: IFGE) { super.visitIFGE(o); visitBranchInstructionAfter(o) }
        override fun visitIFGT(o: IFGT) { super.visitIFGT(o); visitBranchInstructionAfter(o) }
        override fun visitIFLE(o: IFLE) { super.visitIFLE(o); visitBranchInstructionAfter(o) }
        override fun visitIFLT(o: IFLT) { super.visitIFLT(o); visitBranchInstructionAfter(o) }
        override fun visitIFNE(o: IFNE) { super.visitIFNE(o); visitBranchInstructionAfter(o) }
        override fun visitIFNONNULL(o: IFNONNULL) { super.visitIFNONNULL(o); visitBranchInstructionAfter(o) }
        override fun visitIFNULL(o: IFNULL) { super.visitIFNULL(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ACMPEQ(o: IF_ACMPEQ) { super.visitIF_ACMPEQ(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ACMPNE(o: IF_ACMPNE) { super.visitIF_ACMPNE(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPEQ(o: IF_ICMPEQ) { super.visitIF_ICMPEQ(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPGE(o: IF_ICMPGE) { super.visitIF_ICMPGE(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPGT(o: IF_ICMPGT) { super.visitIF_ICMPGT(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPLE(o: IF_ICMPLE) { super.visitIF_ICMPLE(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPLT(o: IF_ICMPLT) { super.visitIF_ICMPLT(o); visitBranchInstructionAfter(o) }
        override fun visitIF_ICMPNE(o: IF_ICMPNE) { super.visitIF_ICMPNE(o); visitBranchInstructionAfter(o) }
        override fun visitJSR(o: JSR) { super.visitJSR(o); visitBranchInstructionAfter(o) }
        override fun visitJSR_W(o: JSR_W) { super.visitJSR_W(o); visitBranchInstructionAfter(o) }
        override fun visitLOOKUPSWITCH(o: LOOKUPSWITCH) { super.visitLOOKUPSWITCH(o); visitBranchInstructionAfter(o) }
        override fun visitTABLESWITCH(o: TABLESWITCH) { super.visitTABLESWITCH(o); visitBranchInstructionAfter(o) }
        // @formatter:on

        override fun visitReturnInstruction(o: ReturnInstruction) {
            if (o is RETURN) {
                returnValues.add(Type.VOID)
            } else {
                returnValues.add(stack.peek())
            }
        }

        override fun visitASTORE(o: ASTORE) {
            super.visitASTORE(o)
            // when storing NBT types in the locals, make sure they have type
            // information attached which can then later be expanded
            locals[o.index] = locals[o.index].ensureTyped()
        }

        override fun visitGETFIELD(o: GETFIELD) {
            super.visitGETFIELD(o)
            // some CompoundTag's are stored as fields instead of locals.
            // it should be enough to attach type info to them when loading
            if (o.getType(cpg).isNbt()) {
                stack.push(stack.pop().ensureTyped())
            }
        }
    }
}
