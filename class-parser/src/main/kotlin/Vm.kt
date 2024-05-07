package de.rubixdev

import org.apache.bcel.Const
import org.apache.bcel.classfile.*
import org.apache.bcel.generic.*
import org.apache.bcel.verifier.structurals.ExecutionVisitor
import org.apache.bcel.verifier.structurals.Frame
import java.io.IOException
import kotlin.math.max

data class MethodCall(
    val method: MethodPointer,
    /**
     * These args should never carry any NBT information, i.e. [Type.untyped] should have been called on each one.
     */
    val args: List<Type>,
    val overrideOptional: Boolean,
) {
    fun toTypeName(): String = "${classToTypeName(method.className)}_${method.name}"
}

/**
 * Stores the modifications that calling a method does to the NBT arguments and what NBT it returns.
 */
data class CallResult(
    val argsNbt: List<NbtElement>,
    val returnNbt: NbtElement?,
) {
    /**
     * Apply this diff to the actual call arguments.
     */
    fun applyTo(actualArgs: List<Type>, pc: Int): NbtElement? {
        // TODO: this assumes that we never call this for methods on NBT classes,
        //  as otherwise the `actualArgs` would contain an extra arg for the instance
        for ((before, diff) in actualArgs.filter { it.isNbt() }.zip(argsNbt)) {
            when (val beforeNbt = before.asNbt()) {
                is NbtCompound -> when (diff) {
                    NbtAny -> {}
                    is NbtCompound -> {
                        val diffCopy = diff.clone() as NbtCompound
                        if (pc < ((before as? TypedTag)?.optionalUntil ?: 0)) {
                            diffCopy.entries.values.forEach { it.optional = true }
                        }
                        beforeNbt.flattened.add(diffCopy)
                    }

                    is NbtBoxed -> beforeNbt.flattened.add(diff)
                    else -> throw RuntimeException("actual arg doesn't match computed diff:\n  before = $before\n  diff = $diff")
                }

                is NbtList -> beforeNbt.merge(diff)
                NbtAny -> {}
                else -> throw RuntimeException("don't know how to handle this type in args:\n  before = $before\n  diff = $diff")
            }
        }
        return returnNbt?.clone()
    }
}

class Vm(private val jarFile: String, private val mcVersion: Int) {
    private val classes = mutableMapOf<String, JavaClass>()
    private val methods = mutableMapOf<MethodCall, CallResult>()
    private val callStack = mutableListOf<MethodCall>()
    private val statics = mutableMapOf<String, Type>()

    /**
     * A set of method calls which were recursive _somewhere_.
     */
    val boxedTypes = mutableSetOf<MethodCall>()

    fun analyzeFrom(method: MethodPointer): NbtCompound {
        val res = call(
            method,
            listOf(ObjectType(method.className)) + Type.getArgumentTypes(method.signature).map { it.ensureTyped() },
            ignoreSuper = true,
        )
        // assume the first argument is the relevant compound tag
        return res.argsNbt[0] as NbtCompound
    }

    private fun call(
        methodPointer: MethodPointer,
        args: List<Type>,
        overrideOptional: Boolean = false,
        ignoreSuper: Boolean = false,
    ): CallResult = MethodCall(methodPointer, args.map { it.untyped() }, overrideOptional).let { thisCall ->
        methods.getOrPut(thisCall) {
            // don't recurse the same method with the same arguments
            if (callStack.contains(thisCall)) {
                val isNew = boxedTypes.add(thisCall)
                require(isNew)
                // instead return boxed versions of compounds resulting from this call
                return CallResult(
                    Type.getArgumentTypes(methodPointer.signature).mapNotNull { it.asNbt() }
                        .map { if (it is NbtCompound) NbtBoxed(thisCall.toTypeName()) else it },
                    Type.getReturnType(methodPointer.signature).asNbt()
                        ?.let { if (it is NbtCompound) NbtBoxed(thisCall.toTypeName()) else it },
                )
            }

            val clazz = getClass(methodPointer.className)
            val method = clazz.methods.find { it.name == methodPointer.name && it.signature == methodPointer.signature }
                ?: return CallResult(
                    Type.getArgumentTypes(methodPointer.signature).mapNotNull { it.asNbt() },
                    Type.getReturnType(methodPointer.signature).asNbt(),
                )

            // strip any existing NBT information from passed args
            val callArgs = args.map { it.untyped().ensureTyped() }
                .onEach { if (it is TypedTag && overrideOptional) it.optionalUntil = Int.MAX_VALUE }

            callStack.add(thisCall)

            // if there's only one CompoundTag argument, and it doesn't have a name yet, give it one based
            // on the class and method names
            val compoundArgs = callArgs.mapNotNull { it.asNbt() }.filterIsInstance<NbtCompound>()
            if (compoundArgs.size == 1) {
                val tag = compoundArgs.first()
                if (tag.name == null) {
                    tag.name = thisCall
                }
            } else if (compoundArgs.size > 1) {
                printWarning("method has more than one compound argument, this is probably handled incorrectly: $methodPointer")
            }

            val insns = InstructionList(method.code.code)
            val runner = MethodRunner(clazz, method, callArgs, ignoreSuper)
            for (insn in insns) {
                runner.visit(insn)
            }
            callStack.removeLast()

            val returnType = Type.getReturnType(methodPointer.signature)
            val returnNbt = returnType.asNbt()?.let {
                var nbt: NbtElement = NbtAny
                for (value in runner.returnValues) {
                    nbt = value.asNbt()?.merge(nbt, MergeStrategy.DifferentDataSet) ?: continue
                }
                // if the return value is a CompoundTag, and it doesn't have a name yet, give it one based
                // on the class and method names
                if (nbt is NbtCompound && nbt.name == null) {
                    nbt.name = thisCall
                }
                nbt
            }
            val argsNbt = callArgs.mapNotNull { it.asNbt() }
            CallResult(argsNbt, returnNbt)
        }
    }

    private fun getClass(name: String) = classes.getOrPut(name) {
        ClassParser(jarFile, "${name.replace('.', '/')}.class").parse().also { initClass(it) }
    }

    private fun getClassOrNull(name: String): JavaClass? {
        return classes.getOrPut(name) {
            try {
                ClassParser(jarFile, "${name.replace('.', '/')}.class").parse().also { initClass(it) }
            } catch (_: IOException) {
                return null
            }
        }
    }

    /**
     * Calls the static initializer of the given class, if there is one, which will initialize static fields.
     */
    private fun initClass(clazz: JavaClass) {
        clazz.methods.find { it.name == "<clinit>" && it.signature == "()V" }?.let { clinit ->
            val insns = InstructionList(clinit.code.code)
            val runner = MethodRunner(clazz, clinit, listOf(), true)
            for (insn in insns) {
                runner.visit(insn)
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
     * These are for example [StringTypeWithValue] or [TypedTag].
     */
    inner class MethodRunner(
        private val clazz: JavaClass,
        private val method: Method,
        args: List<Type>,
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
                locals[idx] = arg.ensureTyped().forLocalsOrStack()
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
                    }.ensureTyped().forLocalsOrStack()
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
                    if (compound !is TypedTag || compound.nbt !is NbtCompound) {
                        printWarning("untyped CompoundTag on stack, cannot save entry")
                    } else {
                        (compound.nbt as NbtCompound).let { nbt ->
                            when (val string = stack.peek(1)) {
                                is StringTypeWithValue -> nbt.put(
                                    string.value,
                                    NbtCompoundEntry(
                                        type,
                                        optional = pc < compound.optionalUntil
                                    )
                                )

                                // this assumes that all strings in the array will be used as keys. That might not
                                // always be true, but it seems to work fine.
                                is StringFromArray -> string.array.filterNotNull().forEach {
                                    nbt.put(it, NbtCompoundEntry(type, optional = pc < compound.optionalUntil))
                                }

                                else -> {
                                    if (
                                        mcVersion < 11700
                                        && clazz.className == "net.minecraft.world.level.block.entity.SignBlockEntity"
                                        && method.name == "save"
                                        && method.signature == "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;"
                                    ) {
                                        // special case for SignBlockEntity before 1.17 which used `"Text" + (i + 1)` as
                                        // keys in a loop with `i in 0..3`.
                                        // That could technically also be handled automatically, but no thank you, this'll
                                        // just be a special case.
                                        // TODO: detect simple for-loops and record all possible values
                                        for (i in 1..4) {
                                            nbt.put("Text$i", NbtCompoundEntry(type, optional = false))
                                        }
                                    } else {
                                        nbt.unknownKeys = type.encompass(nbt.unknownKeys)
                                    }
                                }
                            }
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
                    if (list !is TypedTag || list.nbt !is NbtList) {
                        printWarning("untyped ListTag on stack, cannot save value")
                    } else {
                        (list.nbt as NbtList).add(type)
                    }
                }
            } else if (
                (className == "java.util.Optional" && methodName == "ifPresent")
                || (className == "it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap" && methodName == "forEach")
            ) {
                // lambda methods are resolved in `visitINVOKEDYNAMIC`, now actually call
                // it but mark everything as optional because it's in `Optional.ifPresent`
                val consumer = stack.peek()
                if (consumer is TypeWithLambda) {
                    call(consumer.method, consumer.args, overrideOptional = true).applyTo(consumer.args, pc)
                }
            } else if (className == "com.mojang.datafixers.util.Either" && methodName == "map") {
                val mapLeft = stack.peek(1)
                val mapRight = stack.peek()
                if (mapLeft is TypeWithLambda || mapRight is TypeWithLambda) {
                    val mapReturnType = (mapLeft as? TypeWithLambda)?.method?.signature?.let { Type.getReturnType(it) }
                        ?: Type.getReturnType((mapRight as TypeWithLambda).method.signature)
                    val left = when (mapLeft) {
                        // TODO: perhaps override optional if it modifies an existing tag instead of creating one
                        is TypeWithLambda -> call(mapLeft.method, mapLeft.args).applyTo(mapLeft.args, pc)
                        else -> mapReturnType.asNbt()
                    }
                    val right = when (mapRight) {
                        is TypeWithLambda -> call(mapRight.method, mapRight.args).applyTo(mapRight.args, pc)
                        else -> mapReturnType.asNbt()
                    }

                    val type = if (left == null) {
                        right?.asType() ?: mapReturnType
                    } else if (right == null) {
                        left.asType()
                    } else {
                        NbtEither(left, right).asType()
                    }
                    stack.pop(3)
                    stack.push(type)
                    return
                }
            } else if (invoke(className, methodName, signature, argTypes, returnType, virtual = true, static = false)) {
                return
            }
            // TODO: Codecs and DataResult
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
            // special case for `Entity.saveAsPassenger`:
            // This is currently the only place where `Entity.saveWithoutId` is called,
            // and the only relevant extra thing it does is add the `id` field. Our
            // normal recursing logic won't work for this, as `Entity.saveWithoutId`
            // is an entrypoint. And even if it wasn't, the only type in Rust that will
            // represent _any_ Entity with an id is the `Entity` enum. That means we
            // need this special case to return a `Box<Entity>` where `Entity` isn't
            // the struct used for `Entity.saveWithoutId` but the enum that can represent
            // any entity.
            if (
                className == "net.minecraft.world.entity.Entity"
                && methodName == "saveAsPassenger"
                && signature == "(Lnet/minecraft/nbt/CompoundTag;)Z"
            ) {
                val args = (0..1).map { stack.pop() }.reversed()
                (args[1] as TypedTag).nbt = NbtNestedEntity
                stack.push(Type.INT)
                return true
            }

            // special case for `Entity.saveWithoutId`:
            // Throw an exception to make sure all callers of this were handled specially.
            if (
                className == "net.minecraft.world.entity.Entity"
                && methodName == "saveWithoutId"
                && signature == "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;"
            ) {
                throw RuntimeException("encountered `Entity.saveWithoutId` directly")
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
                val res = call(targetMethod, args).applyTo(args, pc)?.asType() ?: returnType
                // store the result
                if (returnType != Type.VOID) {
                    stack.push(res.forLocalsOrStack())
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
                val lambdaMethod = cp.getConstant<Constant>(lambdaMethodHandle.referenceIndex) as? ConstantMethodref ?: return
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
            // TODO: maybe somehow detect loops that aren't optional
            //  e.g. the Text1, Text2, Text3, and Text4 keys in SignBlockEntity in 1.19.4 shouldn't be optional
            locals.toList().filterIsInstance<TypedTag>()
                .forEach { it.optionalUntil = max(o.target.position, it.optionalUntil) }
            stack.toList().filterIsInstance<TypedTag>()
                .forEach { it.optionalUntil = max(o.target.position, it.optionalUntil) }
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

        override fun visitCHECKCAST(o: CHECKCAST) {
            if (stack.peek().isNbt() && o.getType(cpg).isNbt()) return
            super.visitCHECKCAST(o)
        }

        override fun visitGETSTATIC(o: GETSTATIC) {
            val cp = cpg.constantPool
            val descriptor = cp.constantToString(cp.getConstant(o.index, Const.CONSTANT_Fieldref))
            when (val static = statics[descriptor]) {
                null -> super.visitGETSTATIC(o)
                else -> stack.push(static)
            }
        }

        override fun visitPUTSTATIC(o: PUTSTATIC) {
            val cp = cpg.constantPool
            val descriptor = cp.constantToString(cp.getConstant(o.index, Const.CONSTANT_Fieldref))
            statics[descriptor] = stack.pop()
        }

        override fun visitICONST(o: ICONST) {
            stack.push(IntTypeWithValue(o.value.toInt()))
        }

        override fun visitANEWARRAY(o: ANEWARRAY) {
            val count = stack.peek()
            if (o.getType(cpg).className == Type.STRING.className && count is IntTypeWithValue) {
                stack.pop()
                val list = (0..<count.value).map { null }.toMutableList<String?>()
                stack.push(StringArrayWithValues(list))
            } else {
                super.visitANEWARRAY(o)
            }
        }

        override fun visitAASTORE(o: AASTORE) {
            val value = stack.pop()
            val index = stack.pop()
            val array = stack.pop()
            if (array is StringArrayWithValues && index is IntTypeWithValue && value is StringTypeWithValue) {
                array.values[index.value] = value.value
            }
        }

        override fun visitAALOAD(o: AALOAD) {
            val index = stack.peek()
            val array = stack.peek(1)
            if (array is StringArrayWithValues) {
                stack.pop(2)
                if (index is IntTypeWithValue) {
                    stack.push(array.values[index.value]?.let { StringTypeWithValue(it) }
                        ?: StringFromArray(array.values))
                } else {
                    stack.push(StringFromArray(array.values))
                }
            } else {
                super.visitAALOAD(o)
            }
        }
    }
}
