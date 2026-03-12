#include "W3DDevice/GameClient/W3DAssetManagerExposed.h"
#include "W3DDevice/GameClient/W3DAssetManager.h"

void ReloadAllTextures(void)
{
	W3DAssetManager::Get_Instance()->Release_All_Textures();
}


