import { World } from '@iwsdk/core';
import projectOptions from 'virtual:iwsdk-project';
import { PanelSystem } from './panel.js';
import { FightSystem } from './fight.js';
World.create(document.getElementById('scene-container'), projectOptions).then((world) => {
    world.registerSystem(PanelSystem);
    world.registerSystem(FightSystem);
});
