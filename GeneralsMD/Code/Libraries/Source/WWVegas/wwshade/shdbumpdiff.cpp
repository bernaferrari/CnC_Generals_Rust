#include "dx8fvf.h"
#include "dx8wrapper.h"
#include "assetmgr.h"

#include "shdbumpdiff.h"
#include "shd6bumpdiff.h"
#include "shd7bumpdiff.h"
#include "shd8bumpdiff.h"

#include "editable.h"
#include "shdclassids.h"
#include "shddeffactory.h"
#include "shdinterface.h"

#include "persistfactory.h"
#include "wwhack.h"

DECLARE_FORCE_LINK(BumpDiffShader);
REGISTER_SHDDEF(ShdBumpDiffDefClass,SHDDEF_CLASSID_BUMPDIFF,"Bump Diffuse");

// static member
ShdVersion ShdBumpDiffDefClass::Version;


// Save-Load methods for ShdDefClass
enum 
{
	CHUNKID_VARIABLES =			0x16490450,

	VARID_TEXTURE_NAME =					0x00,
	VARID_BUMP_MAP_NAME,

	VARID_AMBIENT_COLOR,
	VARID_DIFFUSE_COLOR,
	VARID_DIFFUSE_BUMPINESS,
};

ShdBumpDiffDefClass::ShdBumpDiffDefClass() 
:	ShdDefClass(SHDDEF_CLASSID_BUMPDIFF),
	Ambient(1,1,1),
	Diffuse(1,1,1),
	Diffuse_Bumpiness(1,0)
{
	NAMED_TEXTURE_FILENAME_PARAM(ShdBumpDiffDefClass, TextureName, "Base Map", "Targa Files", ".tga");
	NAMED_TEXTURE_FILENAME_PARAM(ShdBumpDiffDefClass, BumpMapName, "Bump Map", "Targa Files", ".tga");

	NAMED_EDITABLE_PARAM(ShdBumpDiffDefClass, ParameterClass::TYPE_COLOR, Ambient, "Ambient");
	NAMED_EDITABLE_PARAM(ShdBumpDiffDefClass, ParameterClass::TYPE_COLOR, Diffuse, "Diffuse");
	
	NAMED_EDITABLE_PARAM(ShdBumpDiffDefClass, ParameterClass::TYPE_FLOAT, Diffuse_Bumpiness.X, "Diffuse Bump Scale");
	NAMED_EDITABLE_PARAM(ShdBumpDiffDefClass, ParameterClass::TYPE_FLOAT, Diffuse_Bumpiness.Y, "Diffuse Bump Bias");
}

ShdBumpDiffDefClass::ShdBumpDiffDefClass(const ShdBumpDiffDefClass& that)
:	ShdDefClass(that),
	Ambient(that.Ambient),
	Diffuse(that.Diffuse),
	Diffuse_Bumpiness(that.Diffuse_Bumpiness)
{
	TextureName=that.TextureName;
	BumpMapName=that.BumpMapName;
}

ShdBumpDiffDefClass::~ShdBumpDiffDefClass()
{
}

bool ShdBumpDiffDefClass::Is_Valid_Config(StringClass &message)
{
	return true;
}

bool ShdBumpDiffDefClass::Save(ChunkSaveClass &csave)
{
	ShdDefClass::Save(csave);

	csave.Begin_Chunk(CHUNKID_VARIABLES);	

		bool retval = true;
	
		// only save the file name
		char fname[_MAX_PATH];

		_splitpath(TextureName,NULL,NULL,fname,NULL);
		strcat(fname,".tga");
		TextureName=fname;

		WRITE_MICRO_CHUNK_WWSTRING(csave, VARID_TEXTURE_NAME, TextureName);

		_splitpath(BumpMapName,NULL,NULL,fname,NULL);
		strcat(fname,".tga");
		BumpMapName=fname;

		WRITE_MICRO_CHUNK_WWSTRING(csave, VARID_BUMP_MAP_NAME, BumpMapName);

		WRITE_MICRO_CHUNK(csave, VARID_AMBIENT_COLOR, Ambient);
		WRITE_MICRO_CHUNK(csave, VARID_DIFFUSE_COLOR, Diffuse);

		WRITE_MICRO_CHUNK(csave, VARID_DIFFUSE_BUMPINESS, Diffuse_Bumpiness);

	csave.End_Chunk();

	return retval;
}

bool ShdBumpDiffDefClass::Load(ChunkLoadClass &cload)
{
	ShdDefClass::Load(cload);

	//	Loop through all the microchunks that define the variables
	while (cload.Open_Chunk()) {
		switch (cload.Cur_Chunk_ID())
		{
		case CHUNKID_VARIABLES:
			while (cload.Open_Micro_Chunk()) 
			{
				switch (cload.Cur_Micro_Chunk_ID()) 
				{
				READ_MICRO_CHUNK_WWSTRING(cload, VARID_TEXTURE_NAME, TextureName);
				READ_MICRO_CHUNK_WWSTRING(cload, VARID_BUMP_MAP_NAME, BumpMapName);

				READ_MICRO_CHUNK(cload, VARID_AMBIENT_COLOR, Ambient);
				READ_MICRO_CHUNK(cload, VARID_DIFFUSE_COLOR, Diffuse);

				READ_MICRO_CHUNK(cload, VARID_DIFFUSE_BUMPINESS, Diffuse_Bumpiness);
				}

				cload.Close_Micro_Chunk();
			}
			break;
		
		default:
			break;
		}

		cload.Close_Chunk();
	}

	return true;
}


void ShdBumpDiffDefClass::Init()
{
	// select shader version
	if ((DX8Wrapper::Get_Current_Caps()->Get_Pixel_Shader_Major_Version())==1 &&
		 (DX8Wrapper::Get_Current_Caps()->Get_Pixel_Shader_Minor_Version())>=1)
	{
		Version.Set(SHDVER_8);
	}
	else if (DX8Wrapper::Get_Current_Caps()->Support_Dot3())
	{
		Version.Set(SHDVER_7);
	}
	else
	{
		Version.Set(SHDVER_6);
	}

	switch (Version.Get())
	{
	case SHDVER_8		: Shd8BumpDiffClass::Init(); break;
	case SHDVER_7		: Shd7BumpDiffClass::Init(); break;
	case SHDVER_6		: Shd6BumpDiffClass::Init(); break;
	}
}

void ShdBumpDiffDefClass::Shutdown()
{
	switch (Version.Get())
	{
	case SHDVER_8		: Shd8BumpDiffClass::Shutdown(); break;
	case SHDVER_7		: Shd7BumpDiffClass::Shutdown(); break;
	case SHDVER_6		: Shd6BumpDiffClass::Shutdown(); break;
	}
}

ShdInterfaceClass* ShdBumpDiffDefClass::Create() const
{
	switch (Version.Get())
	{
	case SHDVER_8		: return new Shd8BumpDiffClass(this); break;
	case SHDVER_7		: return new Shd7BumpDiffClass(this); break;
	case SHDVER_6		: return new Shd6BumpDiffClass(this); break;
	}
	return NULL;
}

