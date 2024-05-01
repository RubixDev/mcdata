package de.rubixdev

import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File
import kotlin.io.path.Path
import kotlin.io.path.createDirectories

fun main(args: Array<String>) {
    val jarPath = args.getOrElse(0) { "mc.jar" }
    val entitiesJsonPath = args.getOrElse(1) { "entities.json" }

    val inputEntityInfo = Json.decodeFromString<InputEntityInfo>(File(entitiesJsonPath).readText())

    val vm = Vm(jarPath)
    val baseNbt = vm.analyzeFrom(MethodPointer(
        "net.minecraft.world.entity.Entity",
        "saveWithoutId",
        "(Lnet/minecraft/nbt/CompoundTag;)Lnet/minecraft/nbt/CompoundTag;",
    ))

    val compoundTypes = mutableListOf<CompoundType>()
    baseNbt.nameCompounds(compoundTypes)
    val entityTypes = mutableListOf(EntityType(
        name = "Entity",
        parent = null,
        nbt = baseNbt,
    ))
    for ((i, entry) in inputEntityInfo.classes.entries.withIndex()) {
        val nbt = vm.analyzeFrom(MethodPointer(entry.key, "addAdditionalSaveData", "(Lnet/minecraft/nbt/CompoundTag;)V"))
        println("${i + 1}/${inputEntityInfo.classes.size}: ${entry.key}")
        // TODO: filter out "empty" types?
        //  e.g. PathfinderMob has no added NBT, so it could be omitted, but then all other types that have
        //  PathfinderMob as their parent would instead have to set the parent of PathfinderMob as their parent.
        nbt.nameCompounds(compoundTypes)
        entityTypes.add(
            EntityType(
                name = classToTypeName(entry.key),
                parent = classToTypeName(entry.value),
                nbt = nbt,
            ),
        )
    }
    val entityInfo = EntityInfo(
        entities = inputEntityInfo.entities.map {
            Entity(
                id = it.id,
                type = classToTypeName(it.className),
                experimental = it.experimental,
            )
        },
        types = entityTypes.sortedBy { it.name },
        compoundTypes = compoundTypes.sortedBy { it.name },
    )
    Path("out").createDirectories()
    File("out/entities.json").writeText(Json.encodeToString(entityInfo))
}

fun classToTypeName(className: String): String = className.split('.').last().split('$').last()
