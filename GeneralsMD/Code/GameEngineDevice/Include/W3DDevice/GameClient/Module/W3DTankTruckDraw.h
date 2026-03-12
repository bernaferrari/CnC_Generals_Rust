// FILE: W3DTankTruckDraw.h ////////////////////////////////////////////////////////////////////////////
// Draw a vehicle with treads and wheels.
// Author: Mark Wilczynski, August 2002
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _W3D_TANKTRUCK_DRAW_H_
#define _W3D_TANKTRUCK_DRAW_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/DrawModule.h"
#include "Common/AudioEventRTS.h"
#include "GameClient/ParticleSys.h"
#include "W3DDevice/GameClient/Module/W3DModelDraw.h"
#include "WW3D2/HAnim.h"
#include "WW3D2/RendObj.h"
#include "WW3D2/Part_Emt.h"

//-------------------------------------------------------------------------------------------------
class W3DTankTruckDrawModuleData : public W3DModelDrawModuleData
{
public:
	AsciiString m_dustEffectName;
	AsciiString m_dirtEffectName;
	AsciiString m_powerslideEffectName;

	AsciiString m_frontLeftTireBoneName;
	AsciiString m_frontRightTireBoneName;
	AsciiString m_rearLeftTireBoneName;
	AsciiString m_rearRightTireBoneName;
	//4 extra tires to support up to 8 tires.
	AsciiString m_midFrontLeftTireBoneName;
	AsciiString m_midFrontRightTireBoneName;
	AsciiString m_midRearLeftTireBoneName;
	AsciiString m_midRearRightTireBoneName;

	Real				m_rotationSpeedMultiplier;
	Real				m_powerslideRotationAddition;

	//Tank data
	AsciiString m_treadDebrisNameLeft;
	AsciiString m_treadDebrisNameRight;

	Real m_treadAnimationRate;	///<amount of tread texture to scroll per sec.  1.0 == full width.
	Real m_treadPivotSpeedFraction;	///<fraction of locomotor speed below which we allow pivoting.
	Real m_treadDriveSpeedFraction;	///<fraction of locomotor speed below which treads stop animating.

	W3DTankTruckDrawModuleData();
	~W3DTankTruckDrawModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class W3DTankTruckDraw : public W3DModelDraw
{

 	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( W3DTankTruckDraw, "W3DTankTruckDraw" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( W3DTankTruckDraw, W3DTankTruckDrawModuleData )
		
public:

	W3DTankTruckDraw( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void setHidden(Bool h);
	virtual void doDrawModule(const Matrix3D* transformMtx);
	virtual void setFullyObscuredByShroud(Bool fullyObscured);
	virtual void reactToGeometryChange() { }

protected:
	virtual void onRenderObjRecreated(void);

protected:
	Bool						m_effectsInitialized;
	Bool						m_wasAirborne;
	Bool						m_isPowersliding;
	/// debris emitters for when tank is moving
	ParticleSystem* m_dustEffect;
	ParticleSystem* m_dirtEffect;
	ParticleSystem* m_powerslideEffect;

	Real						m_frontWheelRotation;
	Real						m_rearWheelRotation;
	Real						m_midFrontWheelRotation;
	Real						m_midRearWheelRotation;

	Int							m_frontLeftTireBone;
	Int							m_frontRightTireBone;
	Int							m_rearLeftTireBone;
	Int							m_rearRightTireBone;
	//4 extra tires to support up to 8 tires
	Int							m_midFrontLeftTireBone;
	Int							m_midFrontRightTireBone;
	Int							m_midRearLeftTireBone;
	Int							m_midRearRightTireBone;

	AudioEventRTS		m_powerslideSound;
	AudioEventRTS		m_landingSound;

	//Tank Data
	/// debris emitters for when tank is moving
	ParticleSystem* m_treadDebrisLeft;
	ParticleSystem* m_treadDebrisRight;

	enum TreadType { TREAD_LEFT, TREAD_RIGHT, TREAD_MIDDLE };	//types of treads for different vehicles
	enum {MAX_TREADS_PER_TANK=4};

	struct TreadObjectInfo
	{
		RenderObjClass	*m_robj;	///<sub-object for tread
		TreadType	m_type;			///<kind of tread
		RenderObjClass::Material_Override m_materialSettings;	///<used to set current uv scroll amount.
	};

	TreadObjectInfo m_treads[MAX_TREADS_PER_TANK];
	Int m_treadCount;

	RenderObjClass *m_prevRenderObj;

	void createEmitters( void );					///< Create particle effects.
	void tossEmitters( void );					///< Create particle effects.
	void enableEmitters( Bool enable );						///< stop creating debris from the tank treads
	void updateBones( void );

	void startMoveDebris( void );												///< start creating debris from the tank treads
	void stopMoveDebris( void );												///< stop creating debris from the tank treads
	void updateTreadObjects(void);												///< update pointers to sub-objects like treads.
	void updateTreadPositions(Real uvDelta);									///< update uv coordinates on each tread
};

#endif // _W3D_TANKTRUCK_DRAW_H_

