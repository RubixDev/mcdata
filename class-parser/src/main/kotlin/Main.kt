package de.rubixdev

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import kotlin.io.path.Path
import kotlin.io.path.createDirectories

fun main(args: Array<String>) {
    val jarPath = args.getOrElse(0) { "mc.jar" }
    val mc = args.getOrElse(1) { "1.20.6" }
    val entitiesJsonPath = args.getOrElse(2) { "entities.json" }
    val blockEntitiesJsonPath = args.getOrElse(3) { "block_entities.json" }

    val mcVersion = (if (mc.count { it == '.' } == 1) "$mc.0" else mc)
        .split('.')
        .joinToString("") { it.padStart(2, '0') }
        .toInt()
    val blockEntitiesMethodName = when {
        mcVersion >= 11800 -> "saveAdditional"
        else -> "save"
    }
    val blockEntitiesMethodSignature = when {
        mcVersion >= 12005 -> "(Lnet/minecraft/nbt/CompoundTag;Lnet/minecraft/core/HolderLookup\$Provider;)V"
        mcVersion >= 11800 -> "(Lnet/minecraft/nbt/CompoundTag;)V"
        else -> "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;"
    }

    val vm = Vm(jarPath, mcVersion)

    println("\u001b[34m>> entities\u001b[0m")
    val inputEntityInfo = Json.decodeFromString<InputEntityInfo>(File(entitiesJsonPath).readText())
    val baseNbt = vm.analyzeFrom(MethodPointer(
        "net.minecraft.world.entity.Entity",
        "saveWithoutId",
        "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;",
    ))

    val compoundTypes = mutableListOf<CompoundType>()
    baseNbt.flatten(vm.boxedTypes)
    baseNbt.nameCompounds(compoundTypes)
    val entityTypes = mutableListOf(EntityType(
        name = "Entity",
        parent = null,
        nbt = baseNbt,
    ))
    for ((i, entry) in inputEntityInfo.classes.entries.withIndex()) {
        print("\u001b[35m> ${i + 1}/${inputEntityInfo.classes.size}: ${entry.key}\u001b[0K\u001b[0m\r")
        val nbt = vm.analyzeFrom(MethodPointer(entry.key, "addAdditionalSaveData", "(Lnet/minecraft/nbt/CompoundTag;)V"))
        // TODO: filter out "empty" types?
        //  e.g. PathfinderMob has no added NBT, so it could be omitted, but then all other types that have
        //  PathfinderMob as their parent would instead have to set the parent of PathfinderMob as their parent.
        nbt.flatten(vm.boxedTypes)
        nbt.nameCompounds(compoundTypes)
        entityTypes.add(
            EntityType(
                name = classToTypeName(entry.key),
                parent = classToTypeName(entry.value),
                nbt = nbt,
            ),
        )
    }
    println()
    val entityInfo = EntityInfo(
        entities = inputEntityInfo.entities.map {
            Entity(
                id = it.id,
                type = classToTypeName(it.className),
                experimental = it.experimental,
            )
        }.sortedBy { it.id },
        types = entityTypes.sortedBy { it.name },
        compoundTypes = compoundTypes.sortedBy { it.name },
    )
    Path("out/$mc").createDirectories()
    File("out/$mc/entities.json").writeText(Json.encodeToString(entityInfo))

    ////// Block Entities //////
    println("\u001b[34m>> block entities\u001b[0m")
    val inputBlockEntityInfo = Json.decodeFromString<InputEntityInfo>(File(blockEntitiesJsonPath).readText())
    compoundTypes.clear()
    vm.boxedTypes.clear()
    val beTypes = mutableListOf(EntityType(
        name = "BlockEntity",
        parent = null,
        nbt = NbtCompound(mutableMapOf(
            "x" to NbtCompoundEntry(NbtInt),
            "y" to NbtCompoundEntry(NbtInt),
            "z" to NbtCompoundEntry(NbtInt),
        )),
    ))

    for ((i, entry) in inputBlockEntityInfo.classes.entries.withIndex()) {
        print("\u001b[35m> ${i + 1}/${inputBlockEntityInfo.classes.size}: ${entry.key}\u001b[0K\u001b[0m\r")
        val nbt = vm.analyzeFrom(MethodPointer(entry.key, blockEntitiesMethodName, blockEntitiesMethodSignature))
        nbt.flatten(vm.boxedTypes)
        nbt.nameCompounds(compoundTypes)
        beTypes.add(
            EntityType(
                name = classToTypeName(entry.key),
                parent = classToTypeName(entry.value),
                nbt = nbt,
            ),
        )
    }
    println()
    val beInfo = EntityInfo(
        entities = inputBlockEntityInfo.entities.map {
            Entity(
                id = it.id,
                type = classToTypeName(it.className),
            )
        }.sortedBy { it.id },
        types = beTypes.sortedBy { it.name },
        compoundTypes = compoundTypes.sortedBy { it.name },
    )
    File("out/$mc/block_entities.json").writeText(Json.encodeToString(beInfo))
}
