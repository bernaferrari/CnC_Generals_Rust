#include "shdlib.h"
#include "assetmgr.h"
#include "shdloader.h"
#include "shdrenderer.h"

void SHD_Init()
{
	ShdRendererClass::Peek_Instance()->Init();
}

void SHD_Shutdown()
{
	ShdRendererClass::Peek_Instance()->Shutdown();
}

void SHD_Init_Shaders()
{
	ShdRendererClass::Init_Shaders();
}

void SHD_Shutdown_Shaders()
{
	ShdRendererClass::Shutdown_Shaders();
}

void SHD_Flush()
{
	ShdRendererClass::Peek_Instance()->Flush();
}

void SHD_Register_Loader()
{
	WW3DAssetManager::Get_Instance()->Register_Prototype_Loader(&_ShdMeshLoader);
//	WW3DAssetManager::Get_Instance()->Register_Prototype_Loader(&_ShdMeshLegacyLoader);
	WW3DAssetManager::Get_Instance()->Register_Prototype_Loader(&_MeshLoader);
}
