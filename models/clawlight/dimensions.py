"""Dimensioned front projection of the actual BREP; no hand-drawn model geometry."""
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.collections import PolyCollection
import numpy as np
from clawlight import parts, electronics, OUT


def main():
    fig,ax=plt.subplots(figsize=(11,9),dpi=160)
    fig.patch.set_facecolor('#f5f3ed'); ax.set_facecolor('#f5f3ed')
    faces=[]
    for n,s in (parts() | electronics()).items():
        v,t=s.tessellate(.05,.1)
        v=np.array([tuple(p) for p in v]); t=np.array(t)
        color='#ee7b16' if n.startswith('01') else '#30343a'
        color={'led_0':'#38ae58','led_1':'#efc12a','led_2':'#d94439'}.get(n,color)
        for f in t:
            faces.append((v[f,1].mean(),v[f][:,[0,2]],color))
    faces.sort(key=lambda f:-f[0])
    ax.add_collection(PolyCollection([f[1] for f in faces],facecolors=[f[2] for f in faces],edgecolors='none',antialiased=False))

    def horizontal(x0,x1,z,label,anchor):
        ax.plot([x0,x0],[anchor,z],color='#889097',lw=.7)
        ax.plot([x1,x1],[anchor,z],color='#889097',lw=.7)
        ax.annotate('',(x0,z),(x1,z),arrowprops={'arrowstyle':'<->','color':'#313944','lw':1})
        ax.text((x0+x1)/2,z-2,label,ha='center',va='top',fontsize=11,color='#313944',backgroundcolor='#f5f3ed')

    def vertical(z0,z1,x,label,anchor):
        ax.plot([anchor,x],[z0,z0],color='#889097',lw=.7)
        ax.plot([anchor,x],[z1,z1],color='#889097',lw=.7)
        ax.annotate('',(x,z0),(x,z1),arrowprops={'arrowstyle':'<->','color':'#313944','lw':1})
        ax.text(x+1.5,(z0+z1)/2,label,ha='left',va='center',fontsize=11,color='#313944',rotation=90)

    horizontal(-30,63,-10,'93 overall width',0)
    horizontal(-25,25,61,'50 body',55)
    horizontal(40,58,83,'18 upright',76)
    vertical(0,76,72,'76 overall height',63)
    vertical(0,55,-37,'55 mascot height',-25)
    vertical(44,56,64,'12 pitch',58)
    ax.set(xlim=(-46,82),ylim=(-19,91),aspect='equal'); ax.axis('off')
    fig.suptitle('CLAW LIGHT  /  DIMENSIONS IN mm',x=.07,y=.97,ha='left',fontsize=18,fontweight='bold')
    fig.text(.07,.925,'Front projection · 40 enclosure depth · 2.4 nominal shell wall · four 5 mm feet',fontsize=10,color='#4d5963')
    fig.text(.07,.035,'LEDs shown: provisional Ø5 packages. Verify actual parts and the fit coupon before printing the enclosure.',fontsize=9,color='#4d5963')
    fig.subplots_adjust(left=.04,right=.96,bottom=.06,top=.9)
    fig.savefig(OUT/'preview'/'dimensions.png',facecolor=fig.get_facecolor())
    fig.savefig(OUT/'preview'/'dimensions.svg',facecolor=fig.get_facecolor())


if __name__=='__main__':
    main()
