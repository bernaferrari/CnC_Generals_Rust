// TerrainTex.h
// Class to generate texture for terrain.
// Author: John Ahlquist, April 2001

#pragma once

#ifndef TERRAINTEX_H
#define TERRAINTEX_H

//#define DO_8STAGE_TERRAIN_PASS		//optimized terrain rendering for Nvidia based cards

#include "WW3D2/Texture.h"
#include "WWMATH/Matrix3d.h"
#include "common/AsciiString.h"

class WorldHeightMap;
#define TILE_OFFSET 8
/** ***********************************************************************
**                             TerrainTextureClass
***************************************************************************/
class TerrainTextureClass : public TextureClass
{
	W3DMPO_GLUE(TerrainTextureClass)
protected:
	virtual void Apply(unsigned int stage);

public:
		/// Create texture for a height map.
		TerrainTextureClass(int height);

		/// Create texture for a height map.
		TerrainTextureClass(int height, int width);

		// just use default destructor. ~TerrainTextureClass(void);
public:
	int update(WorldHeightMap *htMap); ///< Sets the pixels, and returns the actual height of the texture.
	Bool updateFlat(WorldHeightMap *htMap, Int xCell, Int yCell, Int cellWidth, Int pixelsPerCell); ///< Sets the pixels.
	void setLOD(Int LOD);
};


class AlphaTerrainTextureClass : public TextureClass
{
	W3DMPO_GLUE(AlphaTerrainTextureClass)
protected:
		virtual void Apply(unsigned int stage);
public:
		// Create texture for a height map.
		AlphaTerrainTextureClass(TextureClass *pBaseTex );

		// just use default destructor. ~TerrainTextureClass(void);

};

/** ***********************************************************************
**                             AlphaEdgeTextureClass
***************************************************************************/
class AlphaEdgeTextureClass : public TextureClass
{
	W3DMPO_GLUE(AlphaEdgeTextureClass)
protected:
	virtual void Apply(unsigned int stage);
	int update256(WorldHeightMap *htMap);///< Sets the pixels, and returns the actual height of the texture.

public:
		/// Create texture for a height map.
		AlphaEdgeTextureClass(int height, MipCountType mipLevelCount = MIP_LEVELS_3 );

		// just use default destructor. ~TerrainTextureClass(void);
public:
	int update(WorldHeightMap *htMap); ///< Sets the pixels, and returns the actual height of the texture.

};

class LightMapTerrainTextureClass : public TextureClass
{
	W3DMPO_GLUE(LightMapTerrainTextureClass)
protected:
		virtual void Apply(unsigned int stage);

public:
		// Create texture from a height map.
		LightMapTerrainTextureClass( AsciiString name, MipCountType mipLevelCount = MIP_LEVELS_ALL );

		// just use default destructor. 
};

class ScorchTextureClass : public TextureClass
{
	W3DMPO_GLUE(ScorchTextureClass)
protected:
		virtual void Apply(unsigned int stage);

public:
		// Create texture.
		ScorchTextureClass( MipCountType mipLevelCount = MIP_LEVELS_3 );

		// just use default destructor. ~ScorchTextureClass(void);
};

class CloudMapTerrainTextureClass : public TextureClass
{
	W3DMPO_GLUE(CloudMapTerrainTextureClass)
protected:
		virtual void Apply(unsigned int stage);

protected:
		float m_xSlidePerSecond ;	 ///< How far the clouds move per second.
		float m_ySlidePerSecond ;	 ///< How far the clouds move per second.
		int	  m_curTick;
		float m_xOffset;
		float m_yOffset;


public:
		// Create texture from a height map.
		CloudMapTerrainTextureClass( MipCountType mipLevelCount = MIP_LEVELS_ALL );

		// just use default destructor. ~TerrainTextureClass(void);

		void restore(void);
};



#endif //TEXTURE_H
