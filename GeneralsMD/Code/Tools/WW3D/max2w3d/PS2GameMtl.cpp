#include "gamemtl.h"
#include <Max.h>
#include <gport.h>
#include <hsv.h>
#include "dllmain.h"
#include "resource.h"
#include "util.h"



/*****************************************************************
*
*		PS2 GameMtl Class Descriptor
*
*****************************************************************/
Class_ID PS2GameMaterialClassID(0x2ed62ad7, 0x50571dfd);

// This adds W3D PS2 choice to the Max material selector.
class PS2GameMaterialClassDesc:public ClassDesc {

public:
	int				IsPublic()					{ return 1; }
	void *			Create(BOOL loading)		
	{ 
		GameMtl *mtl = new GameMtl(loading);
		mtl->Set_Shader_Type(GameMtl::STE_PS2_SHADER);
		return ((void*)mtl); 
	}
	const TCHAR *	ClassName()					{ return Get_String(IDS_PS2_GAMEMTL); }
	SClass_ID		SuperClassID()				{ return MATERIAL_CLASS_ID; }
	Class_ID 		ClassID()					{ return PS2GameMaterialClassID; }
	const TCHAR* 	Category()					{ return _T("");  }
};

static PS2GameMaterialClassDesc _PS2GameMaterialCD;

ClassDesc * Get_PS2_Game_Material_Desc() { return &_PS2GameMaterialCD;  }
