// FILE: FirestormDynamicGeometryInfoUpdate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:   Update module adds the molestation of a particle system to Geometry changing
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FIRESTORM_DYNAMIC_GEOMETRY_INFO_UPDATE_H_
#define __FIRESTORM_DYNAMIC_GEOMETRY_INFO_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Geometry.h"
#include "GameLogic/Module/DynamicGeometryInfoUpdate.h"

// ------------------------------------------------------------------------------------------------
enum { MAX_FIRESTORM_SYSTEMS = 16 };

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ParticleSystemTemplate;

class FirestormDynamicGeometryInfoUpdateModuleData : public DynamicGeometryInfoUpdateModuleData
{
public:

	FirestormDynamicGeometryInfoUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

	const FXList *m_fxList;
	const ParticleSystemTemplate *m_particleSystem[ MAX_FIRESTORM_SYSTEMS ];
	Real m_particleOffsetZ;
	Real m_scorchSize;
	Real m_delayBetweenDamageFrames;
	Real m_damageAmount;
	Real m_maxHeightForDamage;	// things higher than this above us take no damage

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class FirestormDynamicGeometryInfoUpdate : public DynamicGeometryInfoUpdate
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FirestormDynamicGeometryInfoUpdate, "FirestormDynamicGeometryInfoUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FirestormDynamicGeometryInfoUpdate, FirestormDynamicGeometryInfoUpdateModuleData );

public:

	FirestormDynamicGeometryInfoUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

protected:

	void doDamageScan( void );

	ParticleSystemID m_myParticleSystemID[ MAX_FIRESTORM_SYSTEMS ];
	Bool m_effectsFired;							///< TRUE once the effects have been fired off
	Bool m_scorchPlaced;							///< TRUE once we have placed the scorch mark
	UnsignedInt m_lastDamageFrame;		///< frame we last did damage on
};


#endif

