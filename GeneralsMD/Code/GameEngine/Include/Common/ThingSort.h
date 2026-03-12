// FILE: ThingSort.h //////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   A couple of "buckets" so that we can have things sort easily in the editor
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __THINGSORT_H_
#define __THINGSORT_H_

//-------------------------------------------------------------------------------------------------
enum EditorSortingType
{
	ES_FIRST = 0,

	ES_NONE = ES_FIRST,
	ES_STRUCTURE,
	ES_INFANTRY,
	ES_VEHICLE,
	ES_SHRUBBERY,
	ES_MISC_MAN_MADE,
	ES_MISC_NATURAL,
	ES_DEBRIS,
	ES_SYSTEM,				// game "system" stuff (programmer objects, not objects to put on a map)
	ES_AUDIO,
	ES_TEST,					// for test stuff loaded for the world builder
	ES_FOR_REVIEW,						// awaiting review from the divine messenger
	ES_ROAD,						// road objects...should never actually be in the object panel.
	ES_WAYPOINT,					// waypoint objects...should never actually be in the object panel.

	ES_NUM_SORTING_TYPES      // keep this last

};
#ifdef DEFINE_EDITOR_SORTING_NAMES
static char *EditorSortingNames[] = 
{
	"NONE",
	"STRUCTURE",
	"INFANTRY",
	"VEHICLE",
	"SHRUBBERY",
	"MISC_MAN_MADE",
	"MISC_NATURAL",
	"DEBRIS",
	"SYSTEM",
	"AUDIO",
	"TEST",
	"FOR_REVIEW",
	"ROAD",
	"WAYPOINT",

	NULL
};
#endif

#endif // __THINGSORT_H_

