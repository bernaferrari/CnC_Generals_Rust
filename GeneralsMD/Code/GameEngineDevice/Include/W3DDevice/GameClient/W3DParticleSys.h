// FILE: W3DParticleSys.h /////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3DParticleSys_H_
#define __W3DParticleSys_H_

#include "GameClient/ParticleSys.h"
#include "WW3D2/PointGr.h"
#include "WW3D2/streak.h"
#include "WW3D2/RInfo.h"
#include "WWLib/BitType.h"

//=============================================================================
/** W3D implementation of the game display which is responsible for creating
  * all interaction with the screen and updating the display 
	*/
class W3DParticleSystemManager : public ParticleSystemManager
{

public:
	W3DParticleSystemManager();
	~W3DParticleSystemManager();

	virtual void doParticles(RenderInfoClass &rinfo);
	virtual void queueParticleRender();
	///< returns the number of particles shown on screen per frame
	virtual Int getOnScreenParticleCount() { return m_onScreenParticleCount; }

private:
	enum { MAX_POINTS_PER_GROUP = 512 };

	PointGroupClass *m_pointGroup;							///< the point group that contains all of the particles
	StreakLineClass *m_streakLine;							///< the streak class that contains all of the streaks
	ShareBufferClass<Vector3> *m_posBuffer;			///< array of particle positions
	ShareBufferClass<Vector4> *m_RGBABuffer;		///< array of particle color and alpha
	ShareBufferClass<float> *m_sizeBuffer;			///< array of particle sizes
	ShareBufferClass<uint8> *m_angleBuffer;			///< array of particle orientations
	Bool m_readyToRender;											///< if true, it is OK to render
};

#endif  // end __W3DParticleSys_H_
