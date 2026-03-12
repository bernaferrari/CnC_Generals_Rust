// FILE: W3DSmudge.h /////////////////////////////////////////////////////////

#pragma once

#ifndef _W3DSMUDGE_H_
#define _W3DSMUDGE_H_

#include "GameClient/Smudge.h"
#include "sharebuf.h"

class SmudgeGroupClass;	//forward reference.
class Vector3;
class Vector4;
class TextureClass;
class RenderInfoClass;
class DX8IndexBufferClass;

//#define USE_COPY_RECTS	1	//this was the old method that didn't render to texture. Just copied backbuffer into texture. Slow on Nvidia.

class W3DSmudgeManager : public SmudgeManager
{
public:
	W3DSmudgeManager( void );
	virtual ~W3DSmudgeManager();

	virtual void init(void);
	virtual void reset (void);

	void render (RenderInfoClass &rinfo);
	void ReleaseResources(void);
	void ReAcquireResources(void);

private:
	Bool testHardwareSupport(void);		///<test if video card supports the effect.

	enum { MAX_POINTS_PER_GROUP = 512 };

	SmudgeGroupClass *m_smudgeGroup;							///< the point group that contains all of the particles
	ShareBufferClass<Vector3> *m_posBuffer;			///< array of particle positions
	ShareBufferClass<unsigned int> *m_RGBABuffer;		///< array of particle color and alpha
	ShareBufferClass<float> *m_sizeBuffer;			///< array of particle sizes

#ifdef USE_COPY_RECTS
	TextureClass *m_backgroundTexture;
#endif
	DX8IndexBufferClass	*m_indexBuffer;
	Int m_backBufferWidth;
	Int m_backBufferHeight;
};

#endif	//_W3DSMUDGE_H_