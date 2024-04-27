package com.example;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.FileWriter;
import java.io.IOException;
import java.util.*;
import java.util.stream.Collectors;
import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerLifecycleEvents;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.properties.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class DataExtractor implements ModInitializer {
    public static final Logger LOGGER = LoggerFactory.getLogger("data-extractor");

    @Override
    public void onInitialize() {
        Gson gson = new Gson();

        LOGGER.info("Getting block info");
        Map<String, List<String>> enums = new HashMap<>();
        JsonArray blocks = new JsonArray();
        for (Block block : BuiltInRegistries.BLOCK) {
            JsonObject blockInfo = new JsonObject();
            String blockId = BuiltInRegistries.BLOCK.getKey(block).toString();
            blockInfo.addProperty("id", blockId);

            // check whether the block is experimental
            if (FeatureFlags.isExperimental(block.requiredFeatures())) {
                blockInfo.addProperty("experimental", true);
            }

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
                    } else if (property == BlockStateProperties.VERTICAL_DIRECTION) {
                        enumName = "VerticalDirection";
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

        JsonArray enumList = new JsonArray();
        enums.forEach((key, value) -> {
            JsonObject enumInfo = new JsonObject();
            enumInfo.addProperty("name", key);
            enumInfo.add("values", gson.toJsonTree(value).getAsJsonArray());
            enumList.add(enumInfo);
        });

        LOGGER.info("Getting block property enums info");
        JsonObject blocksJson = new JsonObject();
        blocksJson.add("blocks", blocks);
        blocksJson.add("enums", enumList);

        LOGGER.info("Writing blocks.json");
        try (FileWriter writer = new FileWriter("blocks.json")) {
            writer.write(gson.toJson(blocksJson));
        } catch (IOException e) {
            throw new RuntimeException(e);
        }

        // entities must be spawned to inspect, which requires a world
        ServerLifecycleEvents.SERVER_STARTED.register(server -> {
            // TODO: entity and tile entity stuff

            LOGGER.info("Done!");
            server.halt(false);
        });
    }
}
