// FILE: W3DDebrisDraw.h //////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson
// Desc:   Debris drawing
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __W3D_DEBRIS_DRAW_H_
#define __W3D_DEBRIS_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/GameType.h"
#include "Common/DrawModule.h"
#include "Common/FileSystem.h"	// this is only here to pull in LOAD_TEST_ASSETS

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class RenderObjClass;
class HAnimClass;
class Shadow;
class FXList;

//-------------------------------------------------------------------------------------------------
class W3DDebrisDraw : public DrawModule, public DebrisDrawInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DDebrisDraw, "W3DDebrisDraw" )
	MAKE_STANDARD_MODULE_MACRO( W3DDebrisDraw )

public:

	W3DDebrisDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the draw method
	virtual void doDrawModule(const Matrix3D* transformMtx);

	virtual void setShadowsEnabled(Bool enable);
	virtual void releaseShadows(void) {};	///< we don't care about preserving temporary shadows.	
	virtual void allocateShadows(void) {};	///< we don't care about preserving temporary shadows.

	virtual void setFullyObscuredByShroud(Bool fullyObscured);
	virtual void reactToTransformChange(const Matrix3D* oldMtx, const Coord3D* oldPos, Real oldAngle);
	virtual void reactToGeometryChange() { }

	virtual void setModelName(AsciiString name, Color color, ShadowType t);
	virtual void setAnimNames(AsciiString initial, AsciiString flying, AsciiString final, const FXList* finalFX);

	virtual DebrisDrawInterface* getDebrisDrawInterface() { return this; }
	virtual const DebrisDrawInterface* getDebrisDrawInterface() const { return this; }

private:

	enum AnimStateType
	{
		INITIAL,
		FLYING,
		FINAL,

		STATECOUNT
	};

	AsciiString								m_modelName;
	Color											m_modelColor;
	AsciiString								m_animInitial;
	AsciiString								m_animFlying;
	AsciiString								m_animFinal;
	RenderObjClass*						m_renderObject;										///< W3D Render object for this drawable
	HAnimClass*								m_anims[STATECOUNT];
	const FXList*							m_fxFinal;
	Int												m_state;
	Int												m_frames;
	Bool											m_finalStop;
	Shadow*										m_shadow;													///< Updates/Renders shadows of this object

};

#endif // __W3D_DEBRIS_DRAW_H_

