package com.example;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.FileWriter;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.util.*;
import java.util.stream.Collectors;
import net.fabricmc.api.ModInitializer;
import net.minecraft.core.Registry;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.properties.*;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.Logger;

public class DataExtractor implements ModInitializer {
    public static final Logger LOGGER = LogManager.getLogger("data-extractor");

    @Override
    public void onInitialize() {
        Gson gson = new Gson();

        LOGGER.info("Getting block info");
        Map<String, List<String>> enums = new HashMap<>();
        JsonArray blocks = new JsonArray();
        for (Block block : Registry.BLOCK) {
            JsonObject blockInfo = new JsonObject();
            String blockId = Registry.BLOCK.getKey(block).toString();
            blockInfo.addProperty("id", blockId);

            JsonArray properties = new JsonArray();
            for (Property<?> property : block.defaultBlockState().getProperties()) {
                JsonObject propertyInfo = new JsonObject();
                propertyInfo.addProperty("name", property.getName());

                if (property instanceof BooleanProperty) {
                    propertyInfo.addProperty("type", "bool");
                } else if (property instanceof IntegerProperty) {
                    propertyInfo.addProperty("type", "int");
                    // the min and max fields are private, so we have to find them ourselves from all possible values
                    Collection<Integer> values = ((IntegerProperty) property).getPossibleValues();
                    if (values.isEmpty())
                        LOGGER.error("int property '" + property.getName() + "' of block '" + blockId
                                + "' has no possible values");
                    propertyInfo.addProperty(
                            "min", values.stream().mapToInt(v -> v).min().orElseThrow(NoSuchElementException::new));
                    propertyInfo.addProperty(
                            "max", values.stream().mapToInt(v -> v).max().orElseThrow(NoSuchElementException::new));
                } else if (property instanceof EnumProperty) {
                    propertyInfo.addProperty("type", "enum");

                    // some enums have the same name but not the same values, we have to manually
                    // assign custom names in those cases
                    String enumName;
                    if (property == BlockStateProperties.HORIZONTAL_AXIS) {
                        enumName = "HorizontalAxis";
                    } else if (property == BlockStateProperties.HORIZONTAL_FACING) {
                        enumName = "HorizontalDirection";
                    } else if (property == BlockStateProperties.FACING_HOPPER) {
                        enumName = "HopperDirection";
                    } else if (property == BlockStateProperties.RAIL_SHAPE_STRAIGHT) {
                        enumName = "StraightRailShape";
                    } else {
                        enumName = property.getValueClass().getSimpleName();
                    }
                    propertyInfo.addProperty("enum", enumName);

                    List<String> enumValues = property.getPossibleValues().stream()
                            .map(value -> (value instanceof StringRepresentable)
                                    ? ((StringRepresentable) value).getSerializedName()
                                    : value.toString())
                            .collect(Collectors.toList());
                    if (enums.containsKey(enumName) && !enums.get(enumName).equals(enumValues)) {
                        LOGGER.error("Ambiguous enum name: property '" + property.getName() + "' of '" + blockId
                                + "' has name '" + enumName + "' with the values " + enumValues
                                + ", but another enum with the same name has the values " + enums.get(enumName));
                    } else {
                        enums.put(enumName, enumValues);
                    }
                } else {
                    LOGGER.error("unknown property type '" + property.getClass().getSimpleName() + "' with value type '"
                            + property.getValueClass().getSimpleName() + "'");
                }
                properties.add(propertyInfo);
            }
            blockInfo.add("properties", properties);

            blocks.add(blockInfo);
        }

        LOGGER.info("Getting block property enums info");
        JsonArray enumList = new JsonArray();
        enums.forEach((key, value) -> {
            JsonObject enumInfo = new JsonObject();
            enumInfo.addProperty("name", key);
            enumInfo.add("values", gson.toJsonTree(value).getAsJsonArray());
            enumList.add(enumInfo);
        });

        LOGGER.info("Writing blocks.json");
        JsonObject blocksJson = new JsonObject();
        blocksJson.add("blocks", blocks);
        blocksJson.add("enums", enumList);
        try (FileWriter writer = new FileWriter("blocks.json")) {
            writer.write(gson.toJson(blocksJson));
        } catch (IOException e) {
            throw new RuntimeException(e);
        }

        LOGGER.info("Getting entity info");
        // maps from id to class name
        JsonArray entities = new JsonArray();
        JsonObject superClassMap = new JsonObject();
        for (Field field : EntityType.class.getDeclaredFields()) {
            if (!Modifier.isStatic(field.getModifiers())) continue;

            EntityType<?> entityType;
            try {
                entityType = (EntityType<?>) field.get(null);
            } catch (IllegalAccessException | ClassCastException ignored) {
                // skip non-entity fields
                continue;
            }
            if (entityType == EntityType.PLAYER) {
                LOGGER.info("Skipping player");
                continue;
            }
            Class<?> entityClass = (Class<?>) ((ParameterizedType) field.getGenericType()).getActualTypeArguments()[0];

            JsonObject entityInfo = new JsonObject();
            entityInfo.addProperty("id", EntityType.getKey(entityType).toString());
            entityInfo.addProperty("class", entityClass.getName());
            entities.add(entityInfo);

            Class<?> superclass = entityClass.getSuperclass();
            while (superclass != null && superclass != Object.class && !superClassMap.has(entityClass.getName())) {
                superClassMap.addProperty(entityClass.getName(), superclass.getName());
                entityClass = superclass;
                superclass = entityClass.getSuperclass();
            }
        }

        LOGGER.info("Writing entities.json");
        JsonObject entitiesJson = new JsonObject();
        entitiesJson.add("entities", entities);
        entitiesJson.add("classes", superClassMap);
        try (FileWriter writer = new FileWriter("entities.json")) {
            writer.write(gson.toJson(entitiesJson));
        } catch (IOException e) {
            throw new RuntimeException(e);
        }

        LOGGER.info("Getting block entity info");
        JsonArray blockEntities = new JsonArray();
        JsonObject beSuperClassMap = new JsonObject();
        for (Field field : BlockEntityType.class.getDeclaredFields()) {
            if (!Modifier.isStatic(field.getModifiers())) continue;

            BlockEntityType<?> beType;
            try {
                beType = (BlockEntityType<?>) field.get(null);
            } catch (IllegalAccessException | ClassCastException ignored) {
                // skip non-entity fields
                continue;
            }
            Class<?> beClass = (Class<?>) ((ParameterizedType) field.getGenericType()).getActualTypeArguments()[0];

            JsonObject beInfo = new JsonObject();
            beInfo.addProperty("id", Objects.requireNonNull(BlockEntityType.getKey(beType)).toString());
            beInfo.addProperty("class", beClass.getName());
            blockEntities.add(beInfo);

            Class<?> superclass = beClass.getSuperclass();
            while (superclass != null && superclass != Object.class && !beSuperClassMap.has(beClass.getName())) {
                beSuperClassMap.addProperty(beClass.getName(), superclass.getName());
                beClass = superclass;
                superclass = beClass.getSuperclass();
            }
        }

        LOGGER.info("Writing block_entities.json");
        JsonObject beJson = new JsonObject();
        beJson.add("entities", blockEntities);
        beJson.add("classes", beSuperClassMap);
        try (FileWriter writer = new FileWriter("block_entities.json")) {
            writer.write(gson.toJson(beJson));
        } catch (IOException e) {
            throw new RuntimeException(e);
        }

        LOGGER.info("Done!");
        System.exit(0);
    }
}
