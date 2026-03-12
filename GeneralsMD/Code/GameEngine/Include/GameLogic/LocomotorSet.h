// FILE: LocomotorSet.h /////////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Feb 2002
// Desc: Locomotor Descriptions
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LocomotorSet_H_
#define __LocomotorSet_H_

// no, please do NOT include this.
//#include "GameLogic/Locomotor.h"
#include "Common/GameCommon.h"
#include "Common/STLTypedefs.h"
#include "Common/Snapshot.h"

class Locomotor;
class LocomotorTemplate;

//-------------------------------------------------------------------------------------------------
//
// Note: these values are saved in save files, so you MUST NOT REMOVE OR CHANGE
// existing values!
//
enum LocomotorSurfaceType
{
	LOCOMOTORSURFACE_GROUND			= (1 << 0),									///< clear, unobstructed ground
	LOCOMOTORSURFACE_WATER			= (1 << 1),									///< water area
	LOCOMOTORSURFACE_CLIFF			= (1 << 2),									///< steep altitude change
	LOCOMOTORSURFACE_AIR				= (1 << 3),									///< airborne
	LOCOMOTORSURFACE_RUBBLE			= (1 << 4)									///< building rubble
};

typedef Int LocomotorSurfaceTypeMask;

const LocomotorSurfaceTypeMask NO_SURFACES = (LocomotorSurfaceTypeMask)0x0000;
const LocomotorSurfaceTypeMask ALL_SURFACES = (LocomotorSurfaceTypeMask)0xffff;

#ifdef DEFINE_SURFACECATEGORY_NAMES
static const char *TheLocomotorSurfaceTypeNames[] = 
{
	"GROUND",
	"WATER",
	"CLIFF",
	"AIR",
	"RUBBLE",

	NULL
};
#endif

//-------------------------------------------------------------------------------------------------
typedef std::vector< Locomotor* > LocomotorVector;

//-------------------------------------------------------------------------------------------------
class LocomotorSet : public Snapshot
{
private:
	LocomotorVector						m_locomotors;
	LocomotorSurfaceTypeMask	m_validLocomotorSurfaces;
	Bool											m_downhillOnly;
	
	LocomotorSet(const LocomotorSet& that);
	LocomotorSet& operator=(const LocomotorSet& that);

protected:
	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

public:

	LocomotorSet();
	~LocomotorSet();

	void clear();

	void addLocomotor(const LocomotorTemplate* lt);

	Locomotor* findLocomotor(LocomotorSurfaceTypeMask t);
	
	void xferSelfAndCurLocoPtr(Xfer *xfer, Locomotor** loco);

	inline LocomotorSurfaceTypeMask getValidSurfaces() const { return m_validLocomotorSurfaces; }
	inline Bool isDownhillOnly( void ) const { return m_downhillOnly; };

};

#endif
