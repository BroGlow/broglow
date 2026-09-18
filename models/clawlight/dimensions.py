"""Dimensioned front projection of the actual BREP; no hand-drawn model geometry."""
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.collections import PolyCollection
import numpy as np
from clawlight import (
    DEPTH, LED_PITCH, LED_Z, MASCOT_SCALE, OUT, TOWER_HEIGHT,
    UPRIGHT_LEFT, UPRIGHT_RIGHT, WALL, electronics, parts,
)


def main():
    fig,ax=plt.subplots(figsize=(11,9),dpi=160)
    fig.patch.set_facecolor('#f5f3ed'); ax.set_facecolor('#f5f3ed')
    printed=parts()
    boxes=[s.bounding_box() for s in printed.values()]
    xmin,xmax=min(b.min.X for b in boxes),max(b.max.X for b in boxes)
    mascot_height=55*MASCOT_SCALE
    faces=[]
    for n,s in (printed | electronics()).items():
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

    horizontal(xmin,xmax,-8,f'{xmax-xmin:g} — общая ширина',0)
    horizontal(-25*MASCOT_SCALE,25*MASCOT_SCALE,mascot_height+5,
               f'{50*MASCOT_SCALE:g} — корпус',mascot_height)
    horizontal(UPRIGHT_LEFT,UPRIGHT_RIGHT,TOWER_HEIGHT+6,
               f'{UPRIGHT_RIGHT-UPRIGHT_LEFT:g} — стойка',TOWER_HEIGHT)
    vertical(0,TOWER_HEIGHT,xmax+8,f'{TOWER_HEIGHT:g} — высота',xmax)
    vertical(0,mascot_height,xmin-7,f'{mascot_height:g} — маскот',-25*MASCOT_SCALE)
    vertical(LED_Z[0],LED_Z[1],xmax+2,f'{LED_PITCH:g} — шаг',UPRIGHT_RIGHT)
    ax.set(xlim=(xmin-16,xmax+19),ylim=(-17,TOWER_HEIGHT+14),aspect='equal'); ax.axis('off')
    fig.suptitle('CLAW LIGHT  /  РАЗМЕРЫ В мм',x=.07,y=.97,ha='left',fontsize=18,fontweight='bold')
    fig.text(.07,.925,f'Вид спереди · глубина корпуса {DEPTH:g} · базовая стенка {WALL:g} · четыре ножки по {5*MASCOT_SCALE:g}',fontsize=10,color='#4d5963')
    fig.text(.07,.035,'Измеренные LED: корпус Ø4,75 / фланец Ø5,4. Посадка Ø5,1 проверена купоном; новая сборка требует примерки.',fontsize=9,color='#4d5963')
    fig.subplots_adjust(left=.04,right=.96,bottom=.06,top=.9)
    fig.savefig(OUT/'preview'/'dimensions.png',facecolor=fig.get_facecolor())
    fig.savefig(OUT/'preview'/'dimensions.svg',facecolor=fig.get_facecolor())


if __name__=='__main__':
    main()
