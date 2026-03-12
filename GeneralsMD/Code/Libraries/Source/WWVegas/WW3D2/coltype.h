#if defined(_MSC_VER)
#pragma once
#endif


#ifndef COLTYPE_H
#define COLTYPE_H


/////////////////////////////////////////////////////////////////////////////////////////////////////////////
// 
// Collision 'Types'
//
// This enum defines the collision type bit-field that is used in render object 
// collision detection.
//
// The collision type field in a collision or intersection test is used as a 
// low-level collision mask.  It will be 'AND'ed with the collision type of 
// the render object and will ignore the object unless the result is
// non-zero.  In Commando, we use this to implement separate collision 
// representations for "physical" collisions versus "projectile" 
// collisions.  I.e. we use a very simple mesh for the character's 
// physical collision and a more complex set of meshes for checking whether 
// a bullet hits a person.  This masking system is not meant to be a general 
// "collision grouping" system.  You should use a higher level system for doing
// things like making bullets ignore each other, etc.
//
// One more wrinkle to the system:  The collision type in the render obj
// will always have the LSB set (COLL_TYPE_ALL) so that you can always
// do queries against every piece of geometry in a render obj if desired.
// 
/////////////////////////////////////////////////////////////////////////////////////////////////////////////
enum
{
	COLL_TYPE_ALL	= 0x01,		// perform this test against *EVERYTHING*
	COLL_TYPE_0		= 0x02,		// perform this test against type 0 collision objects
	COLL_TYPE_1		= 0x04,		// perform this test against type 1 collision objects
	COLL_TYPE_2		= 0x08,
	COLL_TYPE_3		= 0x10,
	COLL_TYPE_4		= 0x20,
	COLL_TYPE_5		= 0x40,
	COLL_TYPE_6		= 0x80,

	COLL_TYPE_PHYSICAL	=		COLL_TYPE_0,		// physics collisions
	COLL_TYPE_PROJECTILE =	COLL_TYPE_1,		// projectile collisions
	COLL_TYPE_VIS			=				COLL_TYPE_2,		// "vis node" detection
	COLL_TYPE_CAMERA		=			COLL_TYPE_3,		// camera collision (99% should match physical setting)
	COLL_TYPE_VEHICLE		=		COLL_TYPE_4,		// vehicles will collide with physical and this.
};



#endif

