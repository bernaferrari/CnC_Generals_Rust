// FILE: Statistics.cpp 
/*---------------------------------------------------------------------------*/
/* EA Pacific                                                                */
/* Confidential Information	                                                 */
/* Copyright (C) 2001 - All Rights Reserved                                  */
/* DO NOT DISTRIBUTE                                                         */
/*---------------------------------------------------------------------------*/
/* Project:    RTS3                                                          */
/* File name:  Statistics.cpp                                                */
/* Created:    John K. McDonald, Jr., 4/2/2002                               */
/* Desc:       Statistical functions should live here                        */
/* Revision History:                                                         */
/*		4/2/2002 : Initial creation                                            */
/*---------------------------------------------------------------------------*/

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/Statistics.h"

// Solution taken from http://www.epanorama.net/documents/telecom/ulaw_alaw.html
Real MuLaw(Real valueToRun, Real maxValueForVal, Real mu)
{
	Real testVal = (valueToRun - maxValueForVal / 2) / (maxValueForVal / 2);
	return (sign(testVal) * log(1 + mu * fabs(testVal)) / 
														 log(1 + mu));
}

// from my head. jkmcd
Real Normalize(Real valueToNormalize, Real minRange, Real maxRange)
{
	return ((valueToNormalize - minRange) / (maxRange - minRange));
}

// from my head again. jkmcd
Real NormalizeToRange(Real valueToNormalize, Real minRange, Real maxRange, Real outRangeMin, Real outRangeMax)
{
	return (Normalize(valueToNormalize, minRange, maxRange) * (outRangeMax - outRangeMin)) + outRangeMin;
}
