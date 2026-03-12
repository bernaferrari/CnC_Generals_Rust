// FILE: Statistics.h 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  Statistics.h                                                  */
/* Created:    John K. McDonald, Jr., 4/2/2002                               */
/* Desc:       Common statistical functions                                  */
/* Revision History:                                                         */
/*		4/2/2002 : Initial creation                                            */
/*---------------------------------------------------------------------------*/

#pragma once
#ifndef _H_STATISTICS_
#define _H_STATISTICS_

// INCLUDES ///////////////////////////////////////////////////////////////////
#include "Lib/Basetype.h"

// DEFINES ////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
// FORWARD DECLARATIONS ///////////////////////////////////////////////////////


// valueToRun is the value (between 0 and maxValueForVal) to test.
// mu is designates the steepness of the curve. 
// The return value is a value in [-1, 1]
extern Real MuLaw(Real valueToRun, Real maxValueForVal, Real mu);

// valueToNormalize is a value in minRange..maxRange
// minRange is the smallest value the range contains
// maxRange is the largest value the range contains
// the return is a value in [0, 1].
extern Real Normalize(Real valueToNormalize, Real minRange, Real maxRange);

// same as Normalize, except that output will be in the range [outRangeMin, outRangeMax]
extern Real NormalizeToRange(Real valueToNormalize, Real minRange, Real maxRange, Real outRangeMin, Real outRangeMax);

#endif /* _H_STATISTICS_ */
